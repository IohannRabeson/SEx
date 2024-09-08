use std::path::{Path, PathBuf};

use audio::Audio;
use file_explorer::{ContainerStatus, FileExplorerMessage, FileExplorerModel, NewEntry, NodeId};
use iced::{futures::StreamExt, keyboard, Element, Font, Subscription, Task};
use rfd::AsyncFileDialog;

mod audio;
mod file_explorer;

fn main() -> iced::Result {
    iced::application("SEx", SEx::update, SEx::view)
        .font(include_bytes!("../fonts/SF-Pro.ttf").as_slice())
        .default_font(Font::with_name("SF Pro"))
        .subscription(SEx::subscription)
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
            Self {
                model: None,
                audio: Audio::new(),
            },
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
                    model.add(parent_id, new_entries);
                    model.update_linear_index();
                }
            }
            Message::FileExplorer(FileExplorerMessage::Collapse(id)) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Collapsed);
                    model.update_linear_index();
                }
            }
            Message::FileExplorer(FileExplorerMessage::Expand(id)) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Expanded);
                    model.update_linear_index();
                }
            }
            Message::FileExplorer(FileExplorerMessage::Select(id)) => {
                self.set_selection(id);
            }
            Message::FileExplorer(FileExplorerMessage::SelectNext) => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        if let Some(id) = model.next(current_id) {
                            self.set_selection(Some(id));
                        }
                    }
                }
            }
            Message::FileExplorer(FileExplorerMessage::SelectPrevious) => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        if let Some(id) = model.previous(current_id) {
                            self.set_selection(Some(id));
                        }
                    }
                }
            }
            Message::FileExplorer(FileExplorerMessage::ExpandCollapseCurrent) => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        let mut task = model.expand_collapse(current_id);

                        model.update_linear_index();

                        if task.is_some() {
                            return task.take().unwrap();
                        }
                    }
                }
            }
        }

        Task::none()
    }

    fn set_selection(&mut self, id: Option<NodeId>) {
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

    fn view(&self) -> Element<Message> {
        file_explorer::view(self.model.as_ref())
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, _modifiers| match key {
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(Message::FileExplorer(FileExplorerMessage::SelectNext))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(Message::FileExplorer(FileExplorerMessage::SelectPrevious))
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::FileExplorer(
                FileExplorerMessage::ExpandCollapseCurrent,
            )),
            _ => None,
        })
    }
}

fn is_file_contains_audio(path: impl AsRef<Path>) -> bool {
    let mime_guess = mime_guess::from_path(path);

    mime_guess
        .iter()
        .find(|mime| mime.type_() == mime::AUDIO)
        .is_some()
}

async fn select_existing_directory() -> Option<PathBuf> {
    AsyncFileDialog::new()
        .pick_folder()
        .await
        .map(|fh| fh.path().to_path_buf())
}

async fn load_directory_entries(directory_path: PathBuf) -> Vec<NewEntry> {
    let mut results = Vec::new();

    if let Ok(mut dir_entries) = async_std::fs::read_dir(directory_path).await {
        while let Some(res) = dir_entries.next().await {
            if let Ok(entry) = res {
                if let Ok(metadata) = entry.metadata().await {
                    if metadata.is_dir() {
                        results.push(NewEntry::Directory {
                            path_component: entry.file_name().into_string().unwrap(),
                        });
                    } else if metadata.is_file() {
                        results.push(NewEntry::File {
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
