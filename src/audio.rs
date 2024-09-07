use std::{fs::File, io::BufReader, path::{Path, PathBuf}, sync::{atomic::AtomicBool, mpsc::Sender}, thread::{spawn, JoinHandle}};

pub enum Command {
    Play(PathBuf),
    Stop,
}

pub struct Audio {
    command_sender: Option<Sender<Command>>,
    join_handle: Option<JoinHandle<()>>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.command_sender.take();
        self.join_handle.take().unwrap().join().expect("thread should stop");
    }
}

impl Audio {
    pub fn new() -> Self {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        let join_handle = spawn(move || {
            println!("Starting audio thread");
            let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&stream_handle).unwrap();

            while let Ok(command) = command_receiver.recv() {
                match command {
                    Command::Play(file_path) => {
                        println!("Play {}", file_path.display());

                        sink.clear();
                        
                        if let Ok(file) = File::open(file_path) {
                            if let Ok(source) = rodio::Decoder::new(BufReader::new(file)) {
                                sink.append(source);
                                sink.play();
                            }
                        }
                    },
                    Command::Stop => {
                        sink.stop();
                    },
                }
            }

            println!("Stopping audio thread");
        });

        Self {
            command_sender: Some(command_sender),
            join_handle: Some(join_handle),
        }
    }

    pub fn play(&self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();

        self.command_sender.as_ref().unwrap().send(Command::Play(path)).unwrap();
    }

    pub fn stop(&self) {
        self.command_sender.as_ref().unwrap().send(Command::Stop).unwrap();
    }
}