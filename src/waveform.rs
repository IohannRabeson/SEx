use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use iced::{
    futures::{channel::mpsc, FutureExt, SinkExt, Stream, StreamExt},
    mouse,
    widget::{
        canvas::{self, Cache},
        Canvas,
    },
    Element, Length, Point, Rectangle, Renderer, Size, Subscription, Theme,
};
use rodio::{Decoder, Source};

use crate::Message;

pub enum WaveformCommand {
    LoadFile(PathBuf),
    StopLoading,
}

#[derive(Debug, Clone)]
pub enum WaveformMessage {
    Initialized(mpsc::Sender<WaveformCommand>),
    LoadingStarted(usize),
    LoadingFinished,
    Clear,
    SamplesReady(Vec<i16>),
}

#[derive(Default)]
pub struct Waveform {
    cache: Cache,
    samples: Vec<i16>,
    total_samples: usize,
    command_sender: Option<mpsc::Sender<WaveformCommand>>,
}

enum State {
    Idle,
    Decoding(Box<Decoder<BufReader<File>>>, usize),
}

impl Waveform {
    pub fn show(&mut self, path: impl AsRef<Path>) {
        if let Some(sender) = self.command_sender.as_mut() {
            sender
                .try_send(WaveformCommand::StopLoading)
                .unwrap();

            sender
                .try_send(WaveformCommand::LoadFile(path.as_ref().to_path_buf()))
                .unwrap();
        }
    }

    pub fn clear(&mut self) {
        if let Some(sender) = self.command_sender.as_mut() {
            sender.try_send(WaveformCommand::StopLoading).unwrap();
        }
        self.samples.clear();
    }

    pub fn update(&mut self, message: WaveformMessage) {
        match message {
            WaveformMessage::Initialized(command_sender) => {
                println!("Waveform initialized");
                self.command_sender = Some(command_sender);
            }
            WaveformMessage::LoadingStarted(samples_count) => {
                self.samples.clear();
                self.samples.reserve(samples_count);
                self.total_samples = samples_count;
                println!("Loading started: {samples_count}");
            }
            WaveformMessage::LoadingFinished => {
                println!("Loading finished");
            }
            WaveformMessage::SamplesReady(mut samples) => {
                self.samples.append(&mut samples);
            }
            WaveformMessage::Clear => {
                self.samples.clear();
                self.total_samples = 0;
            }
        }

        self.cache.clear();
    }

    pub fn view(&self) -> Element<Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(waveform_loading).map(Message::Waveform)
    }
}

fn waveform_loading() -> impl Stream<Item = WaveformMessage> {
    iced::stream::channel(8, |mut output| async move {
        let (command_sender, mut command_receiver) = mpsc::channel::<WaveformCommand>(8);

        output
            .send(WaveformMessage::Initialized(command_sender))
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
                State::Decoding(mut decoder, total_samples_count) => {
                    println!("Decoding");
                    const BUFFER_SIZE: usize = 44100;
                    // It's an option because I need to take the buffer when it is filled to avoid cloning it.
                    // It's safe to unwrap it because there always a buffer while decoding.
                    let mut buffer = Some(Vec::with_capacity(BUFFER_SIZE));

                    'outer: for i in 0..total_samples_count {
                        let mut accumulator = 0i32;

                        for c in 0..decoder.channels() {
                            if let Some(WaveformCommand::StopLoading) = command_receiver.next().now_or_never().flatten() {
                                break 'outer
                            }
                            
                            accumulator += match decoder.next() {
                                Some(sample) => sample as i32,
                                None => {
                                    println!("No available samples to decode {} - channel {} - {}", i, c, total_samples_count);
                                    0i32
                                }
                            };
                        }

                        
                        buffer
                            .as_mut()
                            .unwrap()
                            .push((accumulator / decoder.channels() as i32) as i16);

                        if buffer.as_ref().unwrap().len() == BUFFER_SIZE {
                            output
                                .send(WaveformMessage::SamplesReady(buffer.take().unwrap()))
                                .await
                                .unwrap();

                            buffer = Some(Vec::with_capacity(BUFFER_SIZE));
                        }
                    }

                    if !buffer.as_ref().unwrap().is_empty() {
                        output
                            .send(WaveformMessage::SamplesReady(buffer.take().unwrap()))
                            .await
                            .unwrap();
                    }

                    output.send(WaveformMessage::LoadingFinished).await.unwrap();

                    state = State::Idle;
                }
            }
        }
    })
}

async fn process_command(
    command: WaveformCommand,
    output: &mut mpsc::Sender<WaveformMessage>,
) -> State {
    match command {
        WaveformCommand::LoadFile(path) => {
            if let Ok(file) = File::open(path) {
                if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
                    let duration = decoder.total_duration().expect("get total duration");
                    let sample_rate = decoder.sample_rate() as u128;
                    let samples_count = duration.as_nanos() * sample_rate;
                    const DIVISOR: u128 = 1_000_000_000;
                    let samples_count = samples_count / DIVISOR;

                    output
                        .send(WaveformMessage::LoadingStarted(samples_count as usize))
                        .await
                        .unwrap();

                    return State::Decoding(Box::new(decoder), samples_count as usize);
                }
            }

            output.send(WaveformMessage::Clear).await.unwrap();
            
            State::Idle
        }
        WaveformCommand::StopLoading => {
            output.send(WaveformMessage::Clear).await.unwrap();

            State::Idle
        }
    }
}
impl canvas::Program<Message> for Waveform {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let block_size = self.total_samples / frame.width() as usize;
            let palette = theme.palette();

            if block_size > 0 {
                for (index, block) in self.samples.chunks(block_size).enumerate() {
                    if let Some(max) = block.iter().max() {
                        let relative = *max as f32 / i16::MAX as f32;
                        let height = relative * frame.height();

                        frame.fill_rectangle(
                            Point::new(index as f32, (frame.height() - height) / 2f32),
                            Size::new(1f32, height),
                            palette.primary,
                        )
                    }
                }
            }
        });

        vec![geometry]
    }
}
