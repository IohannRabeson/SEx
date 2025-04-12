use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    futures::{channel::mpsc, Stream, StreamExt},
    Subscription, Task,
};
use log::{debug, trace};
use notify::Watcher;

use crate::{file_explorer, file_watcher};

pub enum Command {
    Initialize(Arc<tokio::runtime::Runtime>),
    ResetRootPath(PathBuf),
}

#[derive(Debug, Clone)]
pub enum Message {
    Initialize(mpsc::Sender<Command>),
    Notify(notify::Event),
}

pub struct FileWatcher {
    command_sender: Option<mpsc::Sender<Command>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            command_sender: None,
            runtime: Arc::new(tokio::runtime::Runtime::new().unwrap()),
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
            Message::Initialize(mut sender) => {
                sender
                    .try_send(Command::Initialize(self.runtime.clone()))
                    .unwrap();

                self.command_sender = Some(sender);
            }
            Message::Notify(event) => {
                trace!("{:?}", event);
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
    use iced::futures::SinkExt;

    iced::stream::channel(4, async move |mut output| {
        debug!("Start file watcher subscription");
        let (command_sender, mut command_receiver) = mpsc::channel::<Command>(8);

        output
            .send(crate::Message::FileWatcher(Message::Initialize(
                command_sender,
            )))
            .await
            .unwrap();

        let config = notify::Config::default();
        let mut watcher = None;
        let mut root_path: Option<PathBuf> = None;

        while let Some(command) = command_receiver.next().await {
            match command {
                Command::Initialize(runtime) => {
                    let mut output_handler = output.clone();
                    let event_handler = move |event| {
                        runtime.block_on(async {
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

                    watcher = match notify::RecommendedWatcher::new(event_handler, config) {
                        Ok(watcher) => Some(watcher),
                        Err(error) => {
                            log::error!("Failed to create file watcher: {}", error);
                            None
                        }
                    };
                }
                Command::ResetRootPath(path_buf) => {
                    if let Some(watcher) = watcher.as_mut() {
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
        }
    })
}
