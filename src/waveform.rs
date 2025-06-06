use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::Instant,
};

use iced::widget::canvas;
use iced::{
    event,
    futures::{channel::mpsc, FutureExt, SinkExt, Stream, StreamExt},
    mouse,
    widget::{canvas::Cache, container, MouseArea},
    window, Element, Event, Length, Point, Rectangle, Renderer, Size, Subscription, Task, Theme,
};
use log::debug;
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
    LoadingStarted(Option<usize>),
    LoadingFinished,
    Clear,
    SamplesReady {
        samples: Vec<f32>,
        generation: usize,
    },
    PlayPosition(f32),
    Click,
    CursorMoved(Point),
    Resized,
    BoundsChanged(Option<Rectangle>),
}

#[derive(Default)]
pub struct Waveform {
    waveform_cache: Cache,
    samples: Vec<f32>,
    total_samples: Option<usize>,
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
                debug!("Waveform initialized");
                self.command_sender = Some(command_sender);
            }
            Message::LoadingStarted(samples_count) => {
                self.samples.clear();
                self.total_samples = samples_count;
                self.waveform_cache.clear();

                debug!("Loading started");
            }
            Message::LoadingFinished => {
                debug!("Loading finished");
            }
            Message::SamplesReady {
                mut samples,
                generation,
            } => {
                if self.current_generation == generation {
                    self.samples.append(&mut samples);
                    self.waveform_cache.clear();
                }
            }
            Message::Clear => {
                self.samples.clear();
                self.waveform_cache.clear();
                self.total_samples = None;
            }
            Message::PlayPosition(position) => {
                self.play_position = position;
            }
            Message::Click => {
                if let Some(cursor_position) = self.cursor_position.as_ref() {
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

        Task::none()
    }

    pub fn view(&self) -> Element<crate::Message> {
        MouseArea::new(
            container(canvas(self).width(Length::Fill).height(Length::Fill))
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
                    sample_rate,
                    generation,
                } => {
                    let loading_start_time = Instant::now();
                    let mut total_samples = 0;
                    let buffer_size = sample_rate * 16;
                    debug!("Decoding, buffer size: {}", buffer_size);
                    let mut buffer = Vec::with_capacity(buffer_size);
                    let mut channel = 0;
                    let mut accumulator = 0f32;

                    while let Some(sample) = decoder.next() {
                        if let Some(WaveformCommand::StopLoading) =
                            command_receiver.next().now_or_never().flatten()
                        {
                            buffer.clear();
                            break;
                        }

                        accumulator += sample;
                        channel += 1;

                        if channel == decoder.channels() {
                            buffer.push(accumulator / decoder.channels() as f32);
                            accumulator = 0f32;
                            channel -= decoder.channels();

                            if buffer.len() == buffer_size {
                                total_samples += buffer.len();

                                output
                                    .send(Message::SamplesReady {
                                        samples: buffer.clone(),
                                        generation,
                                    })
                                    .await
                                    .unwrap();

                                buffer.clear();
                            }
                        }
                    }

                    if !buffer.is_empty() {
                        total_samples += buffer.len();

                        output
                            .send(Message::SamplesReady {
                                samples: buffer.clone(),
                                generation,
                            })
                            .await
                            .unwrap();
                    }

                    let duration = Instant::now() - loading_start_time;
                    let duration = duration.as_millis();

                    debug!(
                        "Loading time: {} ms {} samples / ms",
                        duration,
                        if duration == 0 {
                            0
                        } else {
                            total_samples as u128 / duration
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
            match File::open(&path) {
                Ok(file) => {
                    if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
                        let samples_count = decoder.total_duration().map(|duration| {
                            let sample_rate = decoder.sample_rate() as u128;
                            let samples_count = duration.as_nanos() * sample_rate;

                            (samples_count / 1_000_000_000) as usize
                        });
                        let sample_rate = decoder.sample_rate() as usize;

                        debug!("Sample count: {:?}", samples_count);

                        output
                            .send(Message::LoadingStarted(samples_count))
                            .await
                            .unwrap();

                        return State::Decoding {
                            decoder: Box::new(decoder),
                            sample_rate,
                            generation,
                        };
                    }
                }
                Err(error) => log::error!(
                    "Failed to open file '{}' for reading: {}",
                    path.display(),
                    error
                ),
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
        let waveform_geometry = self.waveform_cache.draw(renderer, bounds.size(), |frame| {
            let samples_in_block =
                self.total_samples.unwrap_or(self.samples.len()) / frame.width() as usize;

            // Draw central line
            frame.fill_rectangle(
                Point::new(0.0, frame.height() / 2.0),
                Size::new(frame.width(), 1.0),
                theme.extended_palette().secondary.base.color,
            );

            if samples_in_block > 0 {
                // Draw waveform
                for (index, block) in self.samples.chunks(samples_in_block).enumerate() {
                    if let Some(max) = block
                        .iter()
                        .max_by(|left, right| left.partial_cmp(right).unwrap())
                    {
                        let height = *max * frame.height();

                        frame.fill_rectangle(
                            Point::new(index as f32, (frame.height() - height) / 2f32),
                            Size::new(1f32, height),
                            ui::main_color(theme),
                        )
                    }
                }
            }
        });

        let mut overlay_frame = canvas::Frame::new(renderer, bounds.size());

        if !self.samples.is_empty() {
            // Draw play position
            overlay_frame.fill_rectangle(
                Point::new(self.play_position * overlay_frame.width(), 0f32),
                Size::new(1f32, overlay_frame.height()),
                theme.extended_palette().secondary.base.color,
            );
        }

        // Draw cursor position
        if let Some(cursor_position) = cursor.position_in(bounds) {
            overlay_frame.fill_rectangle(
                Point::new(cursor_position.x, 0f32),
                Size::new(1f32, overlay_frame.height()),
                theme.extended_palette().secondary.base.color,
            );
        }

        vec![waveform_geometry, overlay_frame.into_geometry()]
    }
}

use std::sync::LazyLock;

use crate::{audio, ui};

static WAVEFORM_CONTAINER: LazyLock<container::Id> =
    LazyLock::new(|| container::Id::new("waveform"));

#[cfg(test)]
mod tests {
    use std::{io::Cursor, path::Path, pin::pin};

    use crate::{
        tests::{generate_sine, simulator},
        waveform::{self, waveform_loading, WaveformCommand},
        SEx,
    };
    use iced::futures::{SinkExt, StreamExt};
    use iced_test::Error;
    use rodio::Decoder;

    #[test]
    fn test_waveform() -> Result<(), Error> {
        let (mut app, _task) = SEx::new();

        const SIZE: usize = 1000;
        let buffer = generate_sine(SIZE).collect();

        let _ = app.update(crate::Message::Waveform(
            crate::waveform::Message::LoadingStarted(Some(SIZE)),
        ));
        let _ = app.update(crate::Message::Waveform(
            crate::waveform::Message::SamplesReady {
                samples: buffer,
                generation: 0,
            },
        ));

        let mut ui = simulator(&app);
        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_waveform")?);

        Ok(())
    }

    #[test]
    fn test_waveform_progressive() -> Result<(), Error> {
        let (mut app, _task) = SEx::new();

        const SIZE: usize = 1000;
        let buffer = generate_sine(SIZE).collect();

        let _ = app.update(crate::Message::Waveform(
            crate::waveform::Message::LoadingStarted(None),
        ));
        let _ = app.update(crate::Message::Waveform(
            crate::waveform::Message::SamplesReady {
                samples: buffer,
                generation: 0,
            },
        ));

        let mut ui = simulator(&app);
        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_waveform_progressive")?);

        Ok(())
    }

    const TEST_SINE_MONO: &[u8] = include_bytes!("../audio/test_sine_mono.wav");

    fn load_samples_mono() -> Vec<f32> {
        Decoder::builder()
            .with_data(Cursor::new(TEST_SINE_MONO))
            .build()
            .expect("build decoder")
            .into_iter()
            .collect()
    }

    #[tokio::test]
    async fn test_waveform_loading() {
        let test_file_path = Path::new(file!())
            .parent()
            .expect("get parent")
            .join("../audio/test_sine_mono.wav");
        let mut stream = pin!(waveform_loading());
        let mut init_message = stream.next().await;

        assert!(matches!(
            init_message,
            Some(waveform::Message::Initialized(_))
        ));

        if let Some(waveform::Message::Initialized(command_sender)) = init_message.as_mut() {
            command_sender
                .send(WaveformCommand::LoadFile {
                    path: test_file_path,
                    generation: 0,
                })
                .await
                .unwrap();

            let mut buffer = Vec::new();

            while let Some(message) = stream.next().await {
                match message {
                    waveform::Message::SamplesReady {
                        mut samples,
                        generation,
                    } => {
                        assert_eq!(generation, 0);
                        buffer.append(&mut samples);
                    }
                    waveform::Message::LoadingStarted(_) => (),
                    waveform::Message::LoadingFinished => {
                        assert_eq!(buffer, load_samples_mono());
                        return;
                    }
                    _ => {
                        panic!("Unexpected message '{:?}'", message);
                    }
                }
            }
        } else {
            unreachable!()
        }
    }
}
