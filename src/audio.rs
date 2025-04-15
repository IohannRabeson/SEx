use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::Duration,
};

use details::SourcePicker;
use iced::{
    futures::{
        channel::mpsc::{self, Sender},
        SinkExt, Stream, StreamExt,
    },
    Subscription, Task,
};
use log::debug;
use rodio::{mixer::Mixer, OutputStream, Source};

use crate::{visualization, waveform};

#[derive(Debug, Clone)]
pub enum Message {
    Initialize(Sender<AudioCommand>),
    QueryPosition,
    SetPosition(f32),
}

pub enum AudioCommand {
    Initialize(Mixer),
    Play(PathBuf),
    Stop,
    QueryPosition,
    SetPosition(f32),
}

pub struct Audio {
    command_sender: Option<Sender<AudioCommand>>,
    output_stream: Option<OutputStream>,
}

impl Audio {
    pub fn new() -> Self {
        let output_stream = match rodio::OutputStreamBuilder::open_default_stream() {
            Ok(output_stream) => Some(output_stream),
            Err(error) => {
                log::error!("Unable to create output stream: {}", error);
                None
            }
        };

        Self {
            command_sender: None,
            output_stream,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Initialize(command_sender) => {
                if let Some(output_stream) = self.output_stream.as_ref() {
                    self.command_sender = Some(command_sender);
                    self.send_command(AudioCommand::Initialize(output_stream.mixer().clone()));
                }
            }
            Message::QueryPosition => {
                self.send_command_if_possible(AudioCommand::QueryPosition);
            }
            Message::SetPosition(position) => {
                self.send_command_if_possible(AudioCommand::SetPosition(position));
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        const UI_FRAME_DURATION: Duration = Duration::from_millis(1000 / 60);

        if self.output_stream.is_some() {
            Subscription::batch([
                Subscription::run(run_audio_player),
                iced::time::every(UI_FRAME_DURATION)
                    .map(|_| crate::Message::Audio(Message::QueryPosition)),
            ])
        } else {
            Subscription::none()
        }
    }

    pub fn play(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();

        self.send_command(AudioCommand::Play(path));
    }

    pub fn stop(&mut self) {
        self.send_command(AudioCommand::Stop);
    }

    fn send_command(&mut self, command: AudioCommand) {
        self.command_sender
            .as_mut()
            .expect("command sender initialized")
            .try_send(command)
            .expect("send command");
    }

    fn send_command_if_possible(&mut self, command: AudioCommand) {
        if let Some(command_sender) = self.command_sender.as_mut() {
            let _ = command_sender.try_send(command);
        }
    }
}

fn run_audio_player() -> impl Stream<Item = crate::Message> {
    iced::stream::channel(64, async move |mut output| {
        debug!("Start audio subscription");
        let (command_sender, mut command_receiver) = mpsc::channel::<AudioCommand>(8);

        let mut sink = None;
        let mut mixer = None;

        output
            .send(crate::Message::Audio(Message::Initialize(command_sender)))
            .await
            .unwrap();

        let mut current_file_duration = None;
        let mut current_file_path = None;

        let create_source_output = output.clone();
        let create_source = |file| {
            rodio::Decoder::new(BufReader::new(file))
                .map(|source| SourcePicker::new(source, create_source_output.clone()))
        };

        while let Some(command) = command_receiver.next().await {
            match command {
                AudioCommand::Initialize(new_mixer) => {
                    debug!("Create audio sink");
                    mixer = Some(new_mixer);
                }
                AudioCommand::Play(path) => {
                    // There is a bug where when I change tracks quickly the playing speed starts to change if I keep using the
                    // same Sink again and again. To fix that I'm creating a Sink everytime I play a sound but I should be able to keep the same sink.
                    // https://github.com/IohannRabeson/SEx/issues/8
                    if let Some(mixer) = mixer.as_ref() {
                        sink = Some(rodio::Sink::connect_new(mixer));
                        if let Some(sink) = sink.as_mut() {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(source) = create_source(file) {
                                    current_file_path = Some(path);
                                    current_file_duration = source.total_duration();
                                    sink.append(source);
                                    sink.play();
                                }
                            }
                        }
                    }
                }
                AudioCommand::Stop => {
                    if let Some(sink) = sink.as_mut() {
                        sink.stop();

                        current_file_path = None;
                        current_file_duration = None;

                        // Send an empty audio buffer and zero sample rate to clear visualizers.
                        output
                            .try_send(crate::Message::Visualization(
                                visualization::Message::SampleRateChanged(0),
                            ))
                            .unwrap();

                        output
                            .try_send(crate::Message::Visualization(
                                visualization::Message::AudioBuffer(0, Vec::new()),
                            ))
                            .unwrap();
                    }
                }
                AudioCommand::QueryPosition => {
                    if let Some(sink) = sink.as_mut() {
                        if let Some(duration) = current_file_duration.as_ref() {
                            let position = sink.get_pos().as_secs_f32() / duration.as_secs_f32();

                            output
                                .send(crate::Message::Waveform(waveform::Message::PlayPosition(
                                    position,
                                )))
                                .await
                                .unwrap();
                        }
                    }
                }
                AudioCommand::SetPosition(position) => {
                    if let Some(sink) = sink.as_mut() {
                        if let Some(duration) = current_file_duration.as_ref() {
                            let position =
                                Duration::from_secs_f32(duration.as_secs_f32() * position);
                            if sink.empty() {
                                if let Some(path) = current_file_path.as_ref() {
                                    if let Ok(file) = File::open(path) {
                                        if let Ok(source) = create_source(file) {
                                            sink.append(source);
                                            sink.play();
                                        }
                                    }
                                }
                            }

                            sink.try_seek(position).unwrap();
                        }
                    }
                }
            }
        }
    })
}

mod details {
    use std::time::Duration;

    use iced::futures::channel::mpsc::Sender;
    use rodio::source::SeekError;

    use crate::{visualization, Message};

    pub(crate) struct SourcePicker<S>
    where
        S: rodio::Source + Send + 'static,
    {
        buffer: Vec<f32>,
        buffer_capacity: usize,
        source: S,
        sender: Sender<Message>,
    }

    impl<S> SourcePicker<S>
    where
        S: rodio::Source + Send + 'static,
    {
        pub fn new(source: S, mut sender: Sender<Message>) -> Self {
            let buffer_capacity = source.sample_rate() as usize * source.channels() as usize / 60;

            sender
                .try_send(Message::Visualization(
                    visualization::Message::SampleRateChanged(source.sample_rate() as usize),
                ))
                .unwrap();
            Self {
                buffer: Vec::with_capacity(buffer_capacity),
                buffer_capacity,
                source,
                sender,
            }
        }

        fn push_sample(&mut self, sample: S::Item) {
            self.buffer.push(sample);
            if self.buffer.len() == self.buffer_capacity {
                self.submit_buffer();
            }
        }

        fn submit_buffer(&mut self) {
            self.sender
                .try_send(Message::Visualization(visualization::Message::AudioBuffer(
                    self.source.channels(),
                    self.buffer.to_vec(),
                )))
                .unwrap();

            self.buffer.clear();
        }
    }

    impl<S> rodio::Source for SourcePicker<S>
    where
        S: rodio::Source + Send + 'static,
    {
        fn current_span_len(&self) -> Option<usize> {
            self.source.current_span_len()
        }

        fn channels(&self) -> u16 {
            self.source.channels()
        }

        fn sample_rate(&self) -> u32 {
            self.source.sample_rate()
        }

        fn total_duration(&self) -> Option<std::time::Duration> {
            self.source.total_duration()
        }

        fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
            self.source.try_seek(pos)
        }
    }

    impl<S> Iterator for SourcePicker<S>
    where
        S: rodio::Source + Send + 'static,
    {
        type Item = S::Item;

        fn next(&mut self) -> Option<Self::Item> {
            match self.source.next() {
                Some(sample) => {
                    self.push_sample(sample);
                    Some(sample)
                }
                None => {
                    // Clear the buffer then submit the empty buffer to send a zero value.
                    self.buffer.clear();
                    self.submit_buffer();
                    None
                }
            }
        }
    }
}
