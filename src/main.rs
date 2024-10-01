use std::path::{Path, PathBuf};

use audio::Audio;
use file_explorer::{ContainerStatus, FileExplorerMessage, FileExplorerModel, NewEntry, NodeId};
use iced::{
    futures::StreamExt,
    keyboard,
    widget::{column, pane_grid, PaneGrid},
    Element, Font, Length, Subscription, Task,
};
use rfd::AsyncFileDialog;
use search::{Search, SearchMessage};
use waveform::{Waveform, WaveformMessage};

mod audio;
mod file_explorer;
mod search;
mod waveform;

fn main() -> iced::Result {
    iced::application("SEx Sample Explorer", SEx::update, SEx::view)
        .font(include_bytes!("../fonts/SF-Pro.ttf").as_slice())
        .default_font(Font::with_name("SF Pro"))
        .subscription(SEx::subscription)
        .run_with(SEx::new)
}

#[derive(Debug, Clone)]
enum Message {
    OpenDirectory(Option<PathBuf>),
    FileExplorer(FileExplorerMessage),
    Search(SearchMessage),
    Waveform(WaveformMessage),
    PaneResized(pane_grid::ResizeEvent),
}

enum View {
    Explorer,
    Search,
}

enum PaneState {
    Explorer,
    Waveform,
}

struct SEx {
    model: Option<FileExplorerModel>,
    audio: Audio,
    search: Search,
    view: View,
    panes: pane_grid::State<PaneState>,
    waveform: Waveform,
}

impl SEx {
    fn new() -> (Self, Task<Message>) {
        let (mut panes, waveform_pane) = pane_grid::State::new(PaneState::Waveform);

        if let Some((_, split)) = panes.split(
            pane_grid::Axis::Horizontal,
            waveform_pane,
            PaneState::Explorer,
        ) {
            panes.resize(split, 0.1);
        }

        (
            Self {
                model: None,
                audio: Audio::new(),
                search: Search::new(),
                view: View::Explorer,
                panes,
                waveform: Waveform::default(),
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

                    self.search.set_root_path(path.clone());
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
            Message::Search(message) => {
                return self.search.update(message, &mut self.view);
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::Waveform(message) => {
                self.waveform.update(message);
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
                    self.audio.play(&path);
                    self.waveform.show(&path);
                } else {
                    self.audio.stop();
                    self.waveform.clear();
                }
            } else {
                self.audio.stop();
                self.waveform.clear();
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| match pane {
            PaneState::Explorer => match self.view {
                View::Explorer => column![
                    self.search.view_input(),
                    file_explorer::view(self.model.as_ref())
                ]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
                View::Search => column![self.search.view_input(), self.search.view_results(),]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            },
            PaneState::Waveform => self.waveform.view().into(),
        });

        pane_grid
            .width(Length::Fill)
            .height(Length::Fill)
            .on_resize(8, Message::PaneResized)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
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
            }),
            self.search.subscription(),
            self.waveform.subscription(),
        ])
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
