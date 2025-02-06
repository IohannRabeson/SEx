use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::Duration,
};

use iced::{
    futures::{
        channel::mpsc::{self, Sender},
        SinkExt, Stream, StreamExt,
    },
    Subscription, Task,
};
use rodio::{OutputStream, OutputStreamHandle, Source};

use crate::{waveform::WaveformMessage, Message};

#[derive(Debug, Clone)]
pub enum AudioMessage {
    Initialize(Sender<AudioCommand>),
    QueryPosition,
    SetPosition(f32),
}

pub enum AudioCommand {
    Initialize(OutputStreamHandle),
    Play(PathBuf),
    Stop,
    QueryPosition,
    SetPosition(f32),
}

pub struct Audio {
    command_sender: Option<Sender<AudioCommand>>,
    _output_stream: OutputStream,
    output_stream_handle: OutputStreamHandle,
}

impl Audio {
    pub fn new() -> Self {
        let (output_stream, output_stream_handle) = rodio::OutputStream::try_default().unwrap();

        Self {
            command_sender: None,
            _output_stream: output_stream,
            output_stream_handle,
        }
    }

    pub fn update(&mut self, message: AudioMessage) -> Task<Message> {
        match message {
            AudioMessage::Initialize(command_sender) => {
                self.command_sender = Some(command_sender);
                self.send_command(AudioCommand::Initialize(self.output_stream_handle.clone()));
            }
            AudioMessage::QueryPosition => {
                self.send_command_if_possible(AudioCommand::QueryPosition);
            }
            AudioMessage::SetPosition(position) => {
                self.send_command_if_possible(AudioCommand::SetPosition(position));
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        const UI_FRAME_DURATION: Duration = Duration::from_millis(1000 / 60);

        Subscription::batch([
            Subscription::run(run_audio_player),
            iced::time::every(UI_FRAME_DURATION)
                .map(|_| Message::Audio(AudioMessage::QueryPosition)),
        ])
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

fn run_audio_player() -> impl Stream<Item = Message> {
    iced::stream::channel(1, |mut output| async move {
        println!("Start audio subscription");
        let (command_sender, mut command_receiver) = mpsc::channel::<AudioCommand>(8);

        let mut sink = None;
        let mut output_stream_handle = None;

        output
            .send(Message::Audio(AudioMessage::Initialize(command_sender)))
            .await
            .unwrap();

        let mut current_file_duration = None;
        let mut current_file_path = None;

        while let Some(command) = command_receiver.next().await {
            match command {
                AudioCommand::Initialize(handle) => {
                    println!("Create audio sink");
                    output_stream_handle = Some(handle.clone());
                }
                AudioCommand::Play(path) => {
                    // There is a bug where when I change tracks quickly the playing speed starts to change if I keep using the
                    // same Sink again and again. To fix that I'm creating a Sink everytime I play a sound but I should be able to keep the same sink.
                    // https://github.com/IohannRabeson/SEx/issues/8
                    if let Some(output_stream_handle) = output_stream_handle.as_ref() {
                        sink = rodio::Sink::try_new(&output_stream_handle).ok();
                        if let Some(sink) = sink.as_mut() {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(source) = rodio::Decoder::new(BufReader::new(file)) {
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
                    }
                }
                AudioCommand::QueryPosition => {
                    if let Some(sink) = sink.as_mut() {
                        if let Some(duration) = current_file_duration.as_ref() {
                            let position = sink.get_pos().as_secs_f32() / duration.as_secs_f32();

                            output
                                .send(Message::Waveform(WaveformMessage::PlayPosition(position)))
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
                                        if let Ok(source) =
                                            rodio::Decoder::new(BufReader::new(file))
                                        {
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
