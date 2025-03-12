use std::path::{Path, PathBuf};

use iced::{
    futures::{channel::mpsc, Stream, StreamExt},
    Subscription, Task,
};
use notify::Watcher;

use crate::{file_explorer, file_watcher};

pub enum Command {
    ResetRootPath(PathBuf),
}

#[derive(Debug, Clone)]
pub enum Message {
    Initialize(mpsc::Sender<Command>),
    Notify(notify::Event),
}

pub struct FileWatcher {
    command_sender: Option<mpsc::Sender<Command>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            command_sender: None,
        }
    }

    pub fn watch(&mut self, path: impl AsRef<Path>) {
        if let Some(sender) = self.command_sender.as_mut() {
            sender
                .try_send(Command::ResetRootPath(path.as_ref().to_path_buf()))
                .unwrap()
        }
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Initialize(sender) => {
                self.command_sender = Some(sender);
            }
            Message::Notify(event) => {
                println!("{:?}", event);
                match event.kind {
                    notify::EventKind::Create(_) => {
                        return Task::batch(event.paths.iter().map(|path| {
                            Task::done(crate::Message::FileExplorer(file_explorer::Message::Added(
                                path.clone(),
                            )))
                        }))
                    }
                    notify::EventKind::Remove(_) => {
                        return Task::batch(event.paths.iter().map(|path| {
                            Task::done(crate::Message::FileExplorer(
                                file_explorer::Message::Removed(path.clone()),
                            ))
                        }))
                    }
                    notify::EventKind::Modify(notify::event::ModifyKind::Name(
                        notify::event::RenameMode::Any,
                    )) => {
                        return Task::batch(event.paths.iter().map(|path| match path.exists() {
                            true => Task::done(crate::Message::FileExplorer(
                                file_explorer::Message::Added(path.clone()),
                            )),
                            false => Task::done(crate::Message::FileExplorer(
                                file_explorer::Message::Removed(path.clone()),
                            )),
                        }))
                    }
                    notify::EventKind::Modify(notify::event::ModifyKind::Name(
                        notify::event::RenameMode::From,
                    )) => {
                        return Task::batch(event.paths.iter().map(|path| {
                            Task::done(crate::Message::FileExplorer(
                                file_explorer::Message::Removed(path.clone()),
                            ))
                        }))
                    }
                    notify::EventKind::Modify(notify::event::ModifyKind::Name(
                        notify::event::RenameMode::To,
                    )) => {
                        return Task::batch(event.paths.iter().map(|path| {
                            Task::done(crate::Message::FileExplorer(file_explorer::Message::Added(
                                path.clone(),
                            )))
                        }))
                    }
                    _ => (),
                }
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        Subscription::run(run_watcher)
    }
}

fn run_watcher() -> impl Stream<Item = crate::Message> {
    use async_std::task;
    use iced::futures::SinkExt;

    iced::stream::channel(4, async move |mut output| {
        println!("Start file watcher subscription");
        let (command_sender, mut command_receiver) = mpsc::channel::<Command>(8);

        output
            .send(crate::Message::FileWatcher(Message::Initialize(
                command_sender,
            )))
            .await
            .unwrap();

        let config = notify::Config::default();
        let mut output_handler = output.clone();
        let event_handler = move |event| {
            task::block_on(async {
                match event {
                    Ok(event) => output_handler
                        .send(crate::Message::FileWatcher(file_watcher::Message::Notify(
                            event,
                        )))
                        .await
                        .unwrap(),
                    Err(_) => todo!(),
                }
            });
        };

        let mut watcher =
            notify::RecommendedWatcher::new(event_handler, config).expect("create watcher");
        let mut root_path: Option<PathBuf> = None;

        while let Some(command) = command_receiver.next().await {
            match command {
                Command::ResetRootPath(path_buf) => {
                    if let Some(root_path) = root_path.as_ref() {
                        watcher.unwatch(root_path).unwrap();
                    }
                    watcher
                        .watch(&path_buf, notify::RecursiveMode::Recursive)
                        .unwrap();
                    root_path = Some(path_buf);
                }
            }
        }
    })
}
