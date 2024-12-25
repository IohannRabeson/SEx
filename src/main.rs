use std::path::{Path, PathBuf};

use audio::Audio;
use file_explorer::{FileExplorer, FileExplorerMessage, NewEntry};
use iced::{
    futures::StreamExt,
    keyboard,
    widget::{column, pane_grid, PaneGrid},
    Element, Font, Length, Subscription, Task,
};
use icon_provider::IconProvider;
use rfd::AsyncFileDialog;
use search::{Search, SearchMessage};
use waveform::{Waveform, WaveformMessage};

mod audio;
mod file_explorer;
mod icon_provider;
mod search;
mod ui;
mod waveform;

fn main() -> iced::Result {
    iced::application("SEx - Sample Explorer", SEx::update, SEx::view)
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
    /// Send this message to show the waveform of a file and play it using Task::done.
    /// Send SelectFile(None) to clear the waveform and stop playing audio.
    SelectFile(Option<PathBuf>),
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
    audio: Audio,
    explorer: FileExplorer,
    search: Search,
    view: View,
    panes: pane_grid::State<PaneState>,
    waveform: Waveform,
    icon_provider: IconProvider,
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
                audio: Audio::new(),
                explorer: FileExplorer::default(),
                search: Search::new(),
                view: View::Explorer,
                panes,
                waveform: Waveform::default(),
                icon_provider: IconProvider::default(),
            },
            Task::perform(select_existing_directory(), Message::OpenDirectory),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenDirectory(path) => {
                if let Some(path) = path {
                    assert!(path.is_dir());

                    self.search.set_root_path(path.clone());

                    return self.explorer.set_root_path(&path);
                }
            }
            Message::FileExplorer(message) => {
                return self.explorer.update(message, &self.icon_provider);
            }
            Message::Search(message) => {
                return self
                    .search
                    .update(message, &mut self.view, &self.icon_provider);
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::Waveform(message) => {
                self.waveform.update(message);
            }
            Message::SelectFile(Some(path)) => {
                if path.is_file() && is_file_contains_audio(&path) {
                    self.audio.play(&path);
                    self.waveform.show(&path);
                } else {
                    return Task::done(Message::SelectFile(None));
                }
            }
            Message::SelectFile(None) => {
                self.audio.stop();
                self.waveform.clear();
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let pane_grid = PaneGrid::new(&self.panes, |_id, pane, _is_maximized| match pane {
            PaneState::Explorer => match self.view {
                View::Explorer => column![self.search.view_input(), self.explorer.view(),]
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
    mime_guess::from_path(path)
        .iter()
        .any(|mime| mime.type_() == mime::AUDIO && mime.subtype() != "midi")
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
                            path: entry.path().into(),
                            path_component: entry
                                .file_name()
                                .into_string()
                                .unwrap_or_else(|_| "<conversion error>".to_owned()),
                        });
                    } else if metadata.is_file() {
                        let path: PathBuf = entry.path().into();

                        if is_file_contains_audio(&path) {
                            results.push(NewEntry::File {
                                path,
                                path_component: entry
                                    .file_name()
                                    .into_string()
                                    .unwrap_or_else(|_| "<conversion error>".to_owned()),
                            });
                        }
                    }
                }
            }
        }
    }

    results.sort();

    results
}
