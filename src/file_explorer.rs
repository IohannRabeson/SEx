use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    ops::Deref,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

use iced::{
    widget::{image, row, scrollable, text, Column, MouseArea, Space},
    Element, Length, Task,
};

use crate::{icon_provider::IconProvider, load_directory_entries, ui, Message};

#[derive(Default)]
pub struct FileExplorer {
    model: Option<FileExplorerModel>,
}

impl FileExplorer {
    pub fn set_root_path(&mut self, path: impl AsRef<Path>) -> Task<Message> {
        self.model = Some(FileExplorerModel::new(
            path.as_ref().to_string_lossy().to_string(),
        ));

        let root = self.model.as_ref().unwrap().root_id();

        return Task::perform(load_directory_entries(path.as_ref().to_path_buf()), move |entries| {
            Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(root, entries))
        });
    }

    pub fn view(&self) -> Element<Message> {
        self::view(self.model.as_ref())
    }

    pub fn update(&mut self, message: FileExplorerMessage, icon_provider: &IconProvider) -> Task<Message> {
        match message {
            FileExplorerMessage::RequestLoad(id, path) => {
                return Task::perform(load_directory_entries(path), move |entries| {
                    Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(id, entries))
                });
            }
            FileExplorerMessage::ChildrenLoaded(parent_id, new_entries) => {
                if let Some(model) = self.model.as_mut() {
                    model.add(parent_id, new_entries, icon_provider);
                    model.update_linear_index();
                }
            }
            FileExplorerMessage::Collapse(id) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Collapsed);
                    model.update_linear_index();
                }
            }
            FileExplorerMessage::Expand(id) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Expanded);
                    model.update_linear_index();
                }
            }
            FileExplorerMessage::Select(id) => {
                return self.set_selection(id);
            }
            FileExplorerMessage::SelectNext => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        if let Some(id) = model.next(current_id) {
                            return self.set_selection(Some(id));
                        }
                    }
                }
            }
            FileExplorerMessage::SelectPrevious => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        if let Some(id) = model.previous(current_id) {
                            return self.set_selection(Some(id));
                        }
                    }
                }
            }
            FileExplorerMessage::ExpandCollapseCurrent => {
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

    fn set_selection(&mut self, id: Option<NodeId>) -> Task<Message> {
        if let Some(model) = self.model.as_mut() {
            model.set_selection(id);

            if let Some(id) = id {
                let path = model.path(id);

                return Task::done(Message::SelectFile(Some(path)));
            } else {
                return Task::done(Message::SelectFile(None));
            }
        }

        Task::none()
    }
}

#[derive(Debug, Clone)]
pub enum FileExplorerMessage {
    RequestLoad(NodeId, PathBuf),
    ChildrenLoaded(NodeId, Vec<NewEntry>),
    Collapse(NodeId),
    Expand(NodeId),
    Select(Option<NodeId>),
    SelectNext,
    SelectPrevious,
    ExpandCollapseCurrent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NewEntry {
    Directory {
        path: PathBuf,
        path_component: String,
    },
    File {
        path: PathBuf,
        path_component: String,
    },
}

#[derive(Clone, Copy)]
pub enum ContainerStatus {
    NotLoaded,
    Expanded,
    Collapsed,
    Empty,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct NodeId(usize);

fn view(tree: Option<&FileExplorerModel>) -> Element<Message> {
    const DEPTH_OFFSET: f32 = 16f32;

    let mut main_column = Column::new();

    if let Some(tree) = tree {
        for (id, depth) in tree.linear_visit() {
            if id == &tree.root_id() {
                continue;
            }
            let status = tree.status(*id).unwrap();
            let selectable_part = make_selectable_part(tree, *id);

            let row = row![
                Space::new(Length::Fixed(*depth as f32 * DEPTH_OFFSET), Length::Shrink),
                show_children_control(tree, *id, status),
                Space::new(Length::Fixed(5f32), Length::Shrink),
                selectable_part,
            ];

            main_column = main_column.push(row);
        }
    }
    MouseArea::new(
        scrollable(main_column)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::FileExplorer(FileExplorerMessage::Select(None)))
    .into()
}

fn make_selectable_part(model: &FileExplorerModel, id: NodeId) -> Element<Message> {
    let path_component = model.path_component(id).unwrap();
    let is_selected = model.selection.is_some_and(|selection| selection == id);
    let select_message = Message::FileExplorer(FileExplorerMessage::Select(Some(id)));
    let icon = model.icon(id);

    ui::file_entry(path_component, select_message, icon, is_selected)
}

fn show_children_control(
    tree: &FileExplorerModel,
    id: NodeId,
    status: ContainerStatus,
) -> Element<Message> {
    const COLLAPSED: &str = "▶";
    const EXPANDED: &str = "▼";

    match status {
        ContainerStatus::NotLoaded => {
            let path = tree.path(id);

            MouseArea::new(text(COLLAPSED))
                .on_press(Message::FileExplorer(FileExplorerMessage::RequestLoad(
                    id, path,
                )))
                .into()
        }
        ContainerStatus::Expanded => MouseArea::new(text(EXPANDED))
            .on_press(Message::FileExplorer(FileExplorerMessage::Collapse(id)))
            .into(),
        ContainerStatus::Collapsed => MouseArea::new(text(COLLAPSED))
            .on_press(Message::FileExplorer(FileExplorerMessage::Expand(id)))
            .into(),
        ContainerStatus::Empty => Space::new(Length::Shrink, Length::Shrink).into(),
    }
}

enum Node {
    Root {
        id: NodeId,
        children: Vec<Rc<RefCell<Node>>>,
        path_component: String,
    },
    Directory {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        children: Vec<Rc<RefCell<Node>>>,
        path_component: String,
        status: ContainerStatus,
        icon: Option<image::Handle>,
    },
    File {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        path_component: String,
        icon: Option<image::Handle>,
    },
}

impl Node {
    fn id(&self) -> NodeId {
        match self {
            Node::Root { id, .. } => *id,
            Node::Directory { id, .. } => *id,
            Node::File { id, .. } => *id,
        }
    }

    fn parent(&self) -> Option<NodeId> {
        match self {
            Node::Root { .. } => None,
            Node::Directory { parent, .. } => parent.upgrade().map(|node| node.borrow().id()),
            Node::File { parent, .. } => parent.upgrade().map(|node| node.borrow().id()),
        }
    }

    fn set_parent(&mut self, new_parent: Weak<RefCell<Node>>) {
        match self {
            Node::Root { .. } => {
                panic!("Trying to set parent of the root.")
            }
            Node::Directory { parent, .. } => {
                *parent = new_parent;
            }
            Node::File { parent, .. } => {
                *parent = new_parent;
            }
        }
    }

    fn add_child(&mut self, child: Rc<RefCell<Node>>) {
        match self {
            Node::Root { children, .. } => {
                children.push(child);
            }
            Node::Directory { children, .. } => {
                children.push(child);
            }
            Node::File { .. } => {
                panic!("Trying to add a child to a leaf")
            }
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = NodeId> + '_> {
        match self {
            Node::Root { children, .. } => Box::new(children.iter().map(|node| node.borrow().id())),
            Node::Directory { children, .. } => {
                Box::new(children.iter().map(|node| node.borrow().id()))
            }
            Node::File { .. } => Box::new(std::iter::empty::<NodeId>()),
        }
    }

    fn path_component(&self) -> String {
        match self {
            Node::Root { path_component, .. } => path_component.clone(),
            Node::Directory { path_component, .. } => path_component.clone(),
            Node::File { path_component, .. } => path_component.clone(),
        }
    }

    fn icon(&self) -> Option<image::Handle> {
        match self {
            Node::Root { .. } => None,
            Node::Directory { icon, .. } => icon.clone(),
            Node::File { icon, .. } => icon.clone(),
        }
    }

    fn status(&self) -> ContainerStatus {
        match self {
            Node::Root { .. } => ContainerStatus::Expanded,
            Node::Directory { status, .. } => *status,
            Node::File { .. } => ContainerStatus::Empty,
        }
    }

    fn set_status(&mut self, new_status: ContainerStatus) {
        if let Node::Directory { status, .. } = self {
            *status = new_status;
        }
    }
}

pub struct FileExplorerModel {
    root: Rc<RefCell<Node>>,
    index: BTreeMap<NodeId, Rc<RefCell<Node>>>,
    linear_index: Vec<(NodeId, usize)>,
    next_node_id: usize,
    selection: Option<NodeId>,
}

impl FileExplorerModel {
    pub fn new(root_path_component: String) -> Self {
        let mut next_node_id = 0;
        let root_id = NodeId(next_node_id);
        let root = Rc::new(RefCell::new(Node::Root {
            id: root_id,
            children: Vec::new(),
            path_component: root_path_component,
        }));

        // The root is using the identifier 0.
        next_node_id += 1;

        Self {
            index: BTreeMap::from([(root_id, root.clone())]),
            root,
            next_node_id,
            selection: None,
            linear_index: Vec::new(),
        }
    }

    pub fn root_id(&self) -> NodeId {
        let root = self.root.borrow();

        if let Node::Root { id, .. } = &*root {
            *id
        } else {
            panic!("The root node is not a Root")
        }
    }

    pub fn add(&mut self, parent_id: NodeId, entries: Vec<NewEntry>, icon_provider: &IconProvider) {
        for new_entry in entries {
            match new_entry {
                NewEntry::File {
                    path,
                    path_component,
                } => {
                    let icon = icon_provider.icon(&path).ok();

                    self.add_leaf(parent_id, path_component, icon);
                }
                NewEntry::Directory {
                    path,
                    path_component,
                } => {
                    let icon = icon_provider.icon(&path).ok();

                    self.add_container(parent_id, path_component, icon);
                }
            }
        }

        self.set_status(parent_id, ContainerStatus::Expanded);
    }

    /// Adding a node changes the tree structure so
    /// linear index must be updated using update_linear_index().
    fn add_container(
        &mut self,
        parent: NodeId,
        path_component: String,
        icon: Option<image::Handle>,
    ) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.index.get(&parent).unwrap();
        let mut new_node = Node::Directory {
            id: new_node_id,
            parent: Rc::downgrade(parent_node),
            children: Vec::new(),
            path_component,
            status: ContainerStatus::NotLoaded,
            icon,
        };

        new_node.set_parent(Rc::downgrade(parent_node));

        let new_node = Rc::new(RefCell::new(new_node));

        parent_node.borrow_mut().add_child(new_node.clone());
        self.index.insert(new_node_id, new_node);

        new_node_id
    }

    /// Adding a node changes the tree structure so
    /// linear index must be updated using update_linear_index().
    fn add_leaf(
        &mut self,
        parent: NodeId,
        path_component: String,
        icon: Option<image::Handle>,
    ) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.index.get(&parent).unwrap();
        let mut new_node = Node::File {
            id: new_node_id,
            parent: Rc::downgrade(parent_node),
            path_component,
            icon,
        };

        new_node.set_parent(Rc::downgrade(parent_node));

        let new_node = Rc::new(RefCell::new(new_node));

        parent_node.borrow_mut().add_child(new_node.clone());
        self.index.insert(new_node_id, new_node);

        new_node_id
    }

    /// You must call update_linear_index() to ensure the data is up to date.
    pub fn linear_visit(&self) -> impl Iterator<Item = &(NodeId, usize)> {
        self.linear_index.iter()
    }

    pub fn update_linear_index(&mut self) {
        let initial_depth = 0;
        let mut stack = VecDeque::from([(self.root_id(), initial_depth)]);

        self.linear_index.clear();
        while let Some((current, current_depth)) = stack.pop_front() {
            self.linear_index.push((current, current_depth));

            let current_node = self.index.get(&current).unwrap();

            if matches!(current_node.borrow().status(), ContainerStatus::Expanded) {
                for (index, child_id) in current_node.borrow().children().enumerate() {
                    stack.insert(index, (child_id, current_depth + 1));
                }
            }
        }
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        let node = self.index.get(&id)?;

        node.borrow().parent()
    }

    pub fn next(&self, id: NodeId) -> Option<NodeId> {
        let (index, _) = self
            .linear_index
            .iter()
            .enumerate()
            .find(|(_index, (node_id, _))| node_id == &id)?;

        self.linear_index.get(index + 1).map(|(id, _)| *id)
    }

    pub fn previous(&self, id: NodeId) -> Option<NodeId> {
        let (index, _) = self
            .linear_index
            .iter()
            .enumerate()
            .find(|(_index, (node_id, _))| node_id == &id)?;

        if index == 0 {
            return None;
        }

        self.linear_index.get(index - 1).map(|(id, _)| *id)
    }

    pub fn path_component(&self, id: NodeId) -> Option<String> {
        let node = self.index.get(&id)?;

        Some(node.borrow().path_component())
    }

    fn icon(&self, id: NodeId) -> Option<image::Handle> {
        let node = self.index.get(&id)?;

        node.borrow().icon()
    }

    /// Changing the status changes the structure of the tree so
    /// linear index must be updated using update_linear_index().
    pub fn set_status(&mut self, id: NodeId, status: ContainerStatus) {
        let node = self.index.get(&id).unwrap();

        node.borrow_mut().set_status(status);
    }

    pub fn status(&self, id: NodeId) -> Option<ContainerStatus> {
        let node = self.index.get(&id)?;

        Some(node.borrow().status())
    }

    pub fn expand_collapse(&self, id: NodeId) -> Option<Task<Message>> {
        if let Some(node) = self.index.get(&id) {
            if let Node::Directory { status, .. } = node.borrow().deref() {
                match status {
                    ContainerStatus::Expanded => {
                        return Some(Task::done(Message::FileExplorer(
                            FileExplorerMessage::Collapse(id),
                        )))
                    }
                    ContainerStatus::Collapsed => {
                        return Some(Task::done(Message::FileExplorer(
                            FileExplorerMessage::Expand(id),
                        )))
                    }
                    ContainerStatus::NotLoaded => {
                        let path = self.path(id);

                        return Some(Task::perform(
                            load_directory_entries(path),
                            move |entries| {
                                Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(
                                    id, entries,
                                ))
                            },
                        ));
                    }
                    _ => (),
                }
            }
        }

        None
    }

    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut current = Some(id);
        let mut path_components = Vec::new();

        while let Some(current_id) = current.take() {
            let path_component = self.path_component(current_id).unwrap();

            current = self.parent(current_id);
            path_components.push(path_component);
        }

        let mut result = PathBuf::new();

        for path_component in path_components.iter().rev() {
            result = result.join(path_component);
        }

        result
    }

    pub fn set_selection(&mut self, selection: Option<NodeId>) {
        self.selection = selection;
    }

    pub fn selection(&self) -> Option<NodeId> {
        self.selection
    }
}
