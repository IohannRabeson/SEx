use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::Instant,
};

use iced::{
    event,
    futures::{channel::mpsc, FutureExt, SinkExt, Stream, StreamExt},
    mouse,
    widget::{
        canvas::{self, Cache},
        container, Canvas, MouseArea,
    },
    window, Element, Event, Length, Point, Rectangle, Renderer, Size, Subscription, Task, Theme,
};
use rodio::{Decoder, Source};

pub enum WaveformCommand {
    LoadFile {
        /// Path to the file to load
        path: PathBuf,
        /// Generation number. When a `WaveformMessage::SamplesReady` with a matching generation number
        /// samples data are added to the waveform. This is required to prevent a bug. When loading a long sample, if you
        /// stop the loading (by clicking on a folder), you will have some "delayed" data added to the waveform *after*
        /// clearing it.
        generation: usize,
    },
    StopLoading,
}

#[derive(Debug, Clone)]
pub enum Message {
    Initialized(mpsc::Sender<WaveformCommand>),
    LoadingStarted(usize),
    LoadingFinished,
    Clear,
    SamplesReady { path: Vec<i16>, generation: usize },
    PlayPosition(f32),
    Click,
    CursorMoved(Point),
    Resized,
    BoundsChanged(Option<Rectangle>),
}

#[derive(Default)]
pub struct Waveform {
    cache: Cache,
    samples: Vec<i16>,
    total_samples: usize,
    play_position: f32,
    command_sender: Option<mpsc::Sender<WaveformCommand>>,
    current_generation: usize,
    bounds: Option<Rectangle>,
    cursor_position: Option<Point>,
}

enum State {
    Idle,
    Decoding {
        decoder: Box<Decoder<BufReader<File>>>,
        samples_count: usize,
        sample_rate: usize,
        generation: usize,
    },
}

impl Waveform {
    pub fn show(&mut self, path: impl AsRef<Path>) {
        if let Some(sender) = self.command_sender.as_mut() {
            sender.try_send(WaveformCommand::StopLoading).unwrap();

            self.current_generation += 1;

            sender
                .try_send(WaveformCommand::LoadFile {
                    path: path.as_ref().to_path_buf(),
                    generation: self.current_generation,
                })
                .unwrap();
        }
    }

    pub fn clear(&mut self) {
        if let Some(sender) = self.command_sender.as_mut() {
            self.current_generation += 1;

            sender.try_send(WaveformCommand::StopLoading).unwrap();
        }
        self.samples.clear();
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Initialized(command_sender) => {
                println!("Waveform initialized");
                self.command_sender = Some(command_sender);
            }
            Message::LoadingStarted(samples_count) => {
                self.samples.clear();
                self.samples.reserve(samples_count);
                self.total_samples = samples_count;

                println!("Loading started: {samples_count}");
            }
            Message::LoadingFinished => {
                println!("Loading finished");
            }
            Message::SamplesReady {
                path: mut samples,
                generation,
            } => {
                if self.current_generation == generation {
                    self.samples.append(&mut samples);
                }
            }
            Message::Clear => {
                self.samples.clear();
                self.total_samples = 0;
            }
            Message::PlayPosition(position) => {
                self.play_position = position;
            }
            Message::Click => {
                if let Some(cursor_position) = self.cursor_position.as_ref()
                {
                    if let Some(bounds) = self.bounds.as_ref() {
                        let position = cursor_position.x / bounds.width;

                        return Task::done(crate::Message::Audio(audio::Message::SetPosition(
                            position,
                        )));
                    }
                }
            }
            Message::CursorMoved(position) => {
                self.cursor_position = Some(position);
            }
            Message::Resized => return self.update_bounds(),
            Message::BoundsChanged(rectangle) => {
                self.bounds = rectangle;
            }
        }

        self.cache.clear();

        Task::none()
    }

    pub fn view(&self) -> Element<crate::Message> {
        MouseArea::new(
            container(Canvas::new(self).width(Length::Fill).height(Length::Fill))
                .id(WAVEFORM_CONTAINER.clone()),
        )
        .on_move(|position| crate::Message::Waveform(Message::CursorMoved(position)))
        .on_press(crate::Message::Waveform(Message::Click))
        .into()
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        Subscription::batch([
            Subscription::run(waveform_loading).map(crate::Message::Waveform),
            event::listen_with(|event, _status, _id| -> Option<crate::Message> {
                match event {
                    Event::Window(window::Event::Resized { .. }) => {
                        Some(crate::Message::Waveform(Message::Resized))
                    }
                    _ => None,
                }
            }),
        ])
    }

    pub fn update_bounds(&self) -> Task<crate::Message> {
        container::visible_bounds(WAVEFORM_CONTAINER.clone())
            .map(|rectangle| crate::Message::Waveform(Message::BoundsChanged(rectangle)))
    }
}

fn waveform_loading() -> impl Stream<Item = Message> {
    iced::stream::channel(8, async move |mut output| {
        let (command_sender, mut command_receiver) = mpsc::channel::<WaveformCommand>(8);

        output
            .send(Message::Initialized(command_sender))
            .await
            .unwrap();

        let mut state = State::Idle;

        loop {
            match state {
                State::Idle => {
                    if let Some(command) = command_receiver.next().await {
                        state = process_command(command, &mut output).await;
                    }
                }
                State::Decoding {
                    mut decoder,
                    samples_count,
                    sample_rate,
                    generation,
                } => {
                    let loading_start_time = Instant::now();
                    let buffer_size = sample_rate;
                    println!("Decoding, buffer size: {}", buffer_size);
                    // It's an option because I need to take the buffer when it is filled to avoid cloning it.
                    // It's safe to unwrap it because there always a buffer while decoding.
                    let mut buffer = Some(Vec::with_capacity(buffer_size));

                    'outer: for i in 0..samples_count {
                        let mut accumulator = 0i32;

                        for c in 0..decoder.channels() {
                            if let Some(WaveformCommand::StopLoading) =
                                command_receiver.next().now_or_never().flatten()
                            {
                                buffer.as_mut().unwrap().clear();
                                break 'outer;
                            }

                            accumulator += match decoder.next() {
                                Some(sample) => sample as i32,
                                None => {
                                    println!(
                                        "No available samples to decode {} - channel {} - {}",
                                        i, c, samples_count
                                    );
                                    0i32
                                }
                            };
                        }

                        buffer
                            .as_mut()
                            .unwrap()
                            .push((accumulator / decoder.channels() as i32) as i16);

                        if buffer.as_ref().unwrap().len() == buffer_size {
                            output
                                .send(Message::SamplesReady {
                                    path: buffer.take().unwrap(),
                                    generation,
                                })
                                .await
                                .unwrap();

                            buffer = Some(Vec::with_capacity(buffer_size));
                        }
                    }

                    if !buffer.as_ref().unwrap().is_empty() {
                        output
                            .send(Message::SamplesReady {
                                path: buffer.take().unwrap(),
                                generation,
                            })
                            .await
                            .unwrap();
                    }

                    let duration = Instant::now() - loading_start_time;
                    let duration = duration.as_millis();

                    println!(
                        "Loading time: {} ms {} samples / ms",
                        duration,
                        if duration == 0 {
                            0
                        } else {
                            samples_count as u128 / duration
                        }
                    );

                    output.send(Message::LoadingFinished).await.unwrap();

                    state = State::Idle;
                }
            }
        }
    })
}

async fn process_command(command: WaveformCommand, output: &mut mpsc::Sender<Message>) -> State {
    match command {
        WaveformCommand::LoadFile { path, generation } => {
            if let Ok(file) = File::open(path) {
                if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
                    if let Some(duration) = decoder.total_duration() {
                        let sample_rate = decoder.sample_rate() as u128;
                        let samples_count = duration.as_nanos() * sample_rate;
                        const DIVISOR: u128 = 1_000_000_000;
                        let samples_count = samples_count / DIVISOR;

                        println!("Sample rate: {}", decoder.sample_rate());

                        output
                            .send(Message::LoadingStarted(samples_count as usize))
                            .await
                            .unwrap();

                        return State::Decoding {
                            decoder: Box::new(decoder),
                            samples_count: samples_count as usize,
                            sample_rate: sample_rate as usize,
                            generation,
                        };
                    }
                }
            }

            output.send(Message::Clear).await.unwrap();

            State::Idle
        }
        WaveformCommand::StopLoading => {
            output.send(Message::Clear).await.unwrap();

            State::Idle
        }
    }
}

impl canvas::Program<crate::Message> for Waveform {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let samples_in_block = self.total_samples / frame.width() as usize;

            // Draw central line
            frame.fill_rectangle(
                Point::new(0.0, frame.height() / 2.0),
                Size::new(frame.width(), 1.0),
                theme.extended_palette().secondary.base.color,
            );

            if samples_in_block > 0 {
                // Draw waveform
                for (index, block) in self.samples.chunks(samples_in_block).enumerate() {
                    if let Some(max) = block.iter().max() {
                        let relative = *max as f32 / i16::MAX as f32;
                        let height = relative * frame.height();

                        frame.fill_rectangle(
                            Point::new(index as f32, (frame.height() - height) / 2f32),
                            Size::new(1f32, height),
                            ui::main_color(theme),
                        )
                    }
                }

                // Draw play position
                frame.fill_rectangle(
                    Point::new(self.play_position * frame.width(), 0f32),
                    Size::new(1f32, frame.height()),
                    theme.extended_palette().secondary.base.color,
                );

                // Draw cursor position
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    frame.fill_rectangle(
                        Point::new(cursor_position.x, 0f32),
                        Size::new(1f32, frame.height()),
                        theme.extended_palette().secondary.base.color,
                    );
                }
            }
        });

        vec![geometry]
    }
}

use std::sync::LazyLock;

use crate::{audio, ui};

static WAVEFORM_CONTAINER: LazyLock<container::Id> =
    LazyLock::new(|| container::Id::new("waveform"));
