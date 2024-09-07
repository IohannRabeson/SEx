use std::path::{Path, PathBuf};

use audio::Audio;
use file_explorer::{ContainerStatus, EntryFound, FileExplorerMessage, FileExplorerModel};
use iced::{futures::StreamExt, Element, Font, Task};
use rfd::AsyncFileDialog;

mod file_explorer;
mod audio;

fn main() -> iced::Result {
    iced::application("SEx", SEx::update, SEx::view)
        .font(include_bytes!("../fonts/SF-Pro.ttf").as_slice())
        .default_font(Font::with_name("SF Pro"))
        .run_with(SEx::new)
}

#[derive(Debug, Clone)]
enum Message {
    OpenDirectory(Option<PathBuf>),
    FileExplorer(FileExplorerMessage),
}

struct SEx {
    model: Option<FileExplorerModel>,
    audio: Audio,
}

impl SEx {
    fn new() -> (Self, Task<Message>) {
        (
            Self { model: None, audio: Audio::new() },
            Task::perform(select_existing_directory(), Message::OpenDirectory),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenDirectory(path) => {
                if let Some(path) = path {
                    assert!(path.is_dir());

                    let model = FileExplorerModel::new(path.display().to_string());
                    let root = model.root_id();

                    self.model = Some(model);
                    return Task::perform(load_directory_entries(path), move |entries| {
                        Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(root, entries))
                    });
                }
            }
            Message::FileExplorer(FileExplorerMessage::RequestLoad(id, path)) => {
                return Task::perform(load_directory_entries(path), move |entries| {
                    Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(id, entries))
                });
            }
            Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(parent_id, new_entries)) => {
                if let Some(model) = self.model.as_mut() {
                    for new_entry in new_entries {
                        match new_entry {
                            EntryFound::File { path_component } => {
                                model.add_leaf(parent_id, path_component);
                            }
                            EntryFound::Directory { path_component } => {
                                model.add_container(parent_id, path_component);
                            }
                        }
                    }

                    model.set_status(parent_id, ContainerStatus::Expanded);
                }
            }
            Message::FileExplorer(FileExplorerMessage::Collapse(id)) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Collapsed);
                }
            }
            Message::FileExplorer(FileExplorerMessage::Expand(id)) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Expanded);
                }
            }
            Message::FileExplorer(FileExplorerMessage::Select(id)) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_selection(id);

                    if let Some(id) = id {
                        let path = model.path(id);
                        
                        if path.is_file() && is_file_contains_audio(&path) {
                            self.audio.play(path);
                        } else {
                            self.audio.stop();
                        }
                    } else {
                        self.audio.stop();
                    }
                }
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        file_explorer::view(self.model.as_ref())
    }
}

fn is_file_contains_audio(path: impl AsRef<Path>) -> bool {
    let mime_guess = mime_guess::from_path(path);

    mime_guess.iter().find(|mime|mime.type_() == mime::AUDIO).is_some()
}

async fn select_existing_directory() -> Option<PathBuf> {
    AsyncFileDialog::new()
        .pick_folder()
        .await
        .map(|fh| fh.path().to_path_buf())
}

async fn load_directory_entries(directory_path: PathBuf) -> Vec<EntryFound> {
    let mut results = Vec::new();

    if let Ok(mut dir_entries) = async_std::fs::read_dir(directory_path).await {
        while let Some(res) = dir_entries.next().await {
            if let Ok(entry) = res {
                if let Ok(metadata) = entry.metadata().await {
                    if metadata.is_dir() {
                        results.push(EntryFound::Directory {
                            path_component: entry.file_name().into_string().unwrap(),
                        });
                    } else if metadata.is_file() {
                        results.push(EntryFound::File {
                            path_component: entry.file_name().into_string().unwrap(),
                        });
                    }
                }
            }
        }
    }

    results.sort();

    results
}
