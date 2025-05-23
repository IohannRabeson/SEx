use iced::{
    futures::{
        channel::mpsc::{self, Sender},
        stream::Stream,
        FutureExt, SinkExt, StreamExt,
    },
    widget::{scrollable, svg, text_input, Column},
    Element, Length, Subscription, Task,
};
use log::{debug, trace};
use std::path::PathBuf;

use crate::{display_file, ui, View};

#[derive(Debug, Clone)]
pub enum Message {
    Initialized(Sender<SearchCommand>),
    SearchTextChanged(String),
    SearchStarted,
    SearchFinished,
    FoundResults(Vec<PathBuf>),
    ClearResults,
    Selected(Option<usize>),
    SelectPrevious,
    SelectNext,
}

pub struct Search {
    input: String,
    command_sender: Option<Sender<SearchCommand>>,
    root_path: PathBuf,
    results: Vec<(PathBuf, Option<svg::Handle>)>,
    search_options: SearchOptions,
    selected: Option<usize>,
    directory_icon: svg::Handle,
}

impl Search {
    pub fn new(directory_icon: svg::Handle) -> Self {
        Self {
            input: String::new(),
            command_sender: None,
            root_path: PathBuf::new(),
            results: Vec::new(),
            search_options: SearchOptions::default(),
            selected: None,
            directory_icon,
        }
    }

    pub fn set_root_path(&mut self, path: PathBuf) {
        self.root_path = path;
    }

    pub fn view_input(&self) -> Element<crate::Message> {
        text_input("Search", &self.input)
            .on_input(|text| crate::Message::Search(Message::SearchTextChanged(text)))
            .size(14u32)
            .into()
    }

    pub fn view_results(&self) -> Element<crate::Message> {
        let mut main_column = Column::new();

        for (index, (path, icon)) in self.results.iter().enumerate() {
            let selected = self
                .selected
                .is_some_and(|selected_index| selected_index == index);
            let entry = ui::file_entry(
                path.display(),
                crate::Message::Search(Message::Selected(Some(index))),
                icon.clone(),
                selected,
            );

            main_column = main_column.push(entry);
        }

        scrollable(main_column.width(Length::Fill)).into()
    }

    pub fn update(&mut self, message: Message, view: &mut View) -> Task<crate::Message> {
        match message {
            Message::Initialized(command_sender) => {
                self.command_sender = Some(command_sender);
                debug!("Search initialized");
            }
            Message::SearchTextChanged(text) => {
                self.input = text.clone();
                self.results.clear();

                let command_sender = self.command_sender.as_mut().expect("not initialized");
                if text.is_empty() {
                    command_sender.try_send(SearchCommand::Clear).unwrap();
                    *view = View::Explorer;
                } else {
                    let command = SearchCommand::Search(
                        text,
                        self.root_path.clone(),
                        self.search_options.clone(),
                    );

                    command_sender.try_send(command).unwrap();

                    *view = View::Search;
                };
            }
            Message::FoundResults(results) => {
                self.results.extend(results.into_iter().map(|path| {
                    let icon = if path.is_dir() {
                        Some(self.directory_icon.clone())
                    } else {
                        None
                    };

                    (path, icon)
                }));
            }
            Message::SearchStarted => {
                debug!("Search started");
                self.results.clear();
                *view = View::Search;
            }
            Message::SearchFinished => {
                debug!("Search finished");
            }
            Message::ClearResults => {
                self.results.clear();
            }
            Message::Selected(selected) => {
                self.selected = selected;

                return Task::done(crate::Message::SelectFile(
                    self.selected
                        .map(|selected| self.results[selected].0.clone()),
                ));
            }
            Message::SelectPrevious => {
                if let Some(selected) = self.selected {
                    if selected > 0 {
                        return Task::done(crate::Message::Search(Message::Selected(Some(
                            selected - 1,
                        ))));
                    }
                }
            }
            Message::SelectNext => {
                if let Some(selected) = self.selected {
                    if selected + 1 < self.results.len() {
                        return Task::done(crate::Message::Search(Message::Selected(Some(
                            selected + 1,
                        ))));
                    }
                }
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<crate::Message> {
        Subscription::run(search_new).map(crate::Message::Search)
    }
}

pub enum SearchCommand {
    Search(String, PathBuf, SearchOptions),
    Clear,
}

#[derive(Default, Clone)]
pub struct SearchOptions {
    case_sensitive: bool,
}

fn accept_entry(entry: &tokio::fs::DirEntry, searched: &str, options: &SearchOptions) -> bool {
    if let Some(filename) = entry.file_name().to_str() {
        let accept = if options.case_sensitive {
            filename.contains(searched)
        } else {
            filename.contains(searched)
                || filename.to_lowercase().contains(&searched.to_lowercase())
        };

        return accept && display_file(entry.path());
    }

    false
}

async fn search_filesystem(
    stack: &mut Vec<PathBuf>,
    searched: &str,
    options: &SearchOptions,
) -> Vec<PathBuf> {
    let mut results: Vec<PathBuf> = Vec::new();

    if let Some(current_path) = stack.pop() {
        if let Ok(mut entries) = tokio::fs::read_dir(current_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(metadata) = entry.metadata().await {
                    if metadata.is_dir() || metadata.is_file() {
                        let path = entry.path();

                        if metadata.is_dir() {
                            stack.push(path.clone());
                        }
                        if accept_entry(&entry, searched, options) {
                            results.push(path);
                        }
                    }
                }
            }
        }
    }

    results
}

enum SearchState {
    Idle,
    Search(String, Vec<PathBuf>, SearchOptions),
}
fn search_new() -> impl Stream<Item = Message> {
    iced::stream::channel(20, async move |mut output| {
        let (command_sender, mut command_receiver) = mpsc::channel::<SearchCommand>(16);
        let mut state = SearchState::Idle;

        output
            .send(Message::Initialized(command_sender))
            .await
            .unwrap();

        loop {
            match &mut state {
                SearchState::Idle => {
                    debug!("Waiting for search command");
                    if let Some(SearchCommand::Search(searched, root_directory, options)) =
                        command_receiver.next().await
                    {
                        state = SearchState::Search(searched, vec![root_directory], options);
                    }
                }
                SearchState::Search(searched, directories_to_visit, options) => {
                    if let Some(command) = command_receiver.next().now_or_never().flatten() {
                        match command {
                            SearchCommand::Search(searched, root_directory, options) => {
                                trace!("Search {}", searched);

                                state =
                                    SearchState::Search(searched, vec![root_directory], options);
                                output.send(Message::SearchStarted).await.unwrap();
                            }
                            SearchCommand::Clear => {
                                state = SearchState::Idle;
                                output.send(Message::ClearResults).await.unwrap();
                                debug!("Search cleared");
                            }
                        }
                    } else if directories_to_visit.is_empty() {
                        output.send(Message::SearchFinished).await.unwrap();
                        state = SearchState::Idle;
                    } else {
                        let results =
                            search_filesystem(directories_to_visit, searched, options).await;

                        output.send(Message::FoundResults(results)).await.unwrap();
                    }
                }
            }
        }
    })
}
