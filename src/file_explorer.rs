use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    ffi::{OsStr, OsString},
    ops::Deref,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

use iced::{
    widget::{row, scrollable, svg, text, Column, MouseArea, Space},
    Element, Length, Task,
};

use crate::{load_directory_entries, ui};

pub struct FileExplorer {
    model: Option<FileExplorerModel>,
    directory_icon: svg::Handle,
}

impl FileExplorer {
    pub fn new(directory_icon: svg::Handle) -> Self {
        Self {
            model: None,
            directory_icon,
        }
    }

    pub fn set_root_path(&mut self, path: impl AsRef<Path>) -> Task<crate::Message> {
        self.model = Some(FileExplorerModel::new(
            path.as_ref().as_os_str().to_os_string(),
        ));

        let root = self.model.as_ref().unwrap().root_id();

        Task::perform(
            load_directory_entries(path.as_ref().to_path_buf()),
            move |entries| crate::Message::FileExplorer(Message::ChildrenLoaded(root, entries)),
        )
    }

    pub fn view(&self) -> Element<crate::Message> {
        self::view(self.model.as_ref(), self.directory_icon.clone())
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::RequestLoad(id, path) => {
                return Task::perform(load_directory_entries(path), move |entries| {
                    crate::Message::FileExplorer(Message::ChildrenLoaded(id, entries))
                });
            }
            Message::ChildrenLoaded(parent_id, new_entries) => {
                if let Some(model) = self.model.as_mut() {
                    model.add(parent_id, new_entries);
                    model.update_linear_index();
                }
            }
            Message::Collapse(id) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Collapsed);
                    model.update_linear_index();
                }
            }
            Message::Expand(id) => {
                if let Some(model) = self.model.as_mut() {
                    model.set_status(id, ContainerStatus::Expanded);
                    model.update_linear_index();
                }
            }
            Message::Select(id) => {
                return self.set_selection(id);
            }
            Message::SelectNext => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        if let Some(id) = model.next(current_id) {
                            return self.set_selection(Some(id));
                        }
                    }
                }
            }
            Message::SelectPrevious => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(current_id) = model.selection() {
                        if let Some(id) = model.previous(current_id) {
                            return self.set_selection(Some(id));
                        }
                    }
                }
            }
            Message::ExpandCollapseCurrent => {
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
            Message::Removed(path_buf) => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(id) = model.node(&path_buf) {
                        model.remove(id);
                    }
                }
            }
            Message::Added(path_buf) => {
                if let Some(model) = self.model.as_mut() {
                    if let Some(parent_path) = path_buf.parent() {
                        if let Some(id) = model.node(parent_path) {
                            return Task::perform(
                                load_directory_entries(parent_path.to_path_buf()),
                                move |entries| {
                                    crate::Message::FileExplorer(Message::ChildrenLoaded(
                                        id, entries,
                                    ))
                                },
                            );
                        }
                    }
                }
            }
        }

        Task::none()
    }

    fn set_selection(&mut self, id: Option<NodeId>) -> Task<crate::Message> {
        if let Some(model) = self.model.as_mut() {
            model.set_selection(id);

            return Task::done(crate::Message::SelectFile(id.map(|id| model.path(id))));
        }

        Task::none()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    RequestLoad(NodeId, PathBuf),
    ChildrenLoaded(NodeId, Vec<NewEntry>),
    Collapse(NodeId),
    Expand(NodeId),
    Select(Option<NodeId>),
    SelectNext,
    SelectPrevious,
    ExpandCollapseCurrent,
    Removed(PathBuf),
    Added(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NewEntry {
    Directory { path_component: OsString },
    File { path_component: OsString },
}

impl NewEntry {
    pub fn path_component(&self) -> &OsStr {
        match self {
            NewEntry::Directory { path_component, .. } => path_component,
            NewEntry::File { path_component, .. } => path_component,
        }
    }
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

#[cfg(test)]
impl NodeId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

fn view(tree: Option<&FileExplorerModel>, directory_icon: svg::Handle) -> Element<crate::Message> {
    const DEPTH_OFFSET: f32 = 20f32;

    let mut main_column = Column::new();

    if let Some(tree) = tree {
        for (id, depth) in tree.linear_visit() {
            if id == &tree.root_id() {
                continue;
            }
            let status = tree.status(*id).unwrap();
            let selectable_part = make_selectable_part(tree, *id, directory_icon.clone());
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
    .on_press(crate::Message::FileExplorer(Message::Select(None)))
    .into()
}

fn make_selectable_part(
    model: &FileExplorerModel,
    id: NodeId,
    directory_icon: svg::Handle,
) -> Element<crate::Message> {
    let path_component = model.path_component(id).unwrap();
    let icon = if model.is_directory(id) {
        Some(directory_icon)
    } else {
        None
    };
    let is_selected = model.selection.is_some_and(|selection| selection == id);
    let select_message = crate::Message::FileExplorer(Message::Select(Some(id)));

    ui::file_entry(
        path_component.into_string().unwrap(),
        select_message,
        icon,
        is_selected,
    )
}

fn show_children_control(
    tree: &FileExplorerModel,
    id: NodeId,
    status: ContainerStatus,
) -> Element<crate::Message> {
    const COLLAPSED: &str = "▶";
    const EXPANDED: &str = "▼";

    match status {
        ContainerStatus::NotLoaded => {
            let path = tree.path(id);

            MouseArea::new(text(COLLAPSED))
                .on_press(crate::Message::FileExplorer(Message::RequestLoad(id, path)))
                .into()
        }
        ContainerStatus::Expanded => MouseArea::new(text(EXPANDED))
            .on_press(crate::Message::FileExplorer(Message::Collapse(id)))
            .into(),
        ContainerStatus::Collapsed => MouseArea::new(text(COLLAPSED))
            .on_press(crate::Message::FileExplorer(Message::Expand(id)))
            .into(),
        ContainerStatus::Empty => Space::new(Length::Shrink, Length::Shrink).into(),
    }
}

enum Node {
    Root {
        id: NodeId,
        children: Vec<Rc<RefCell<Node>>>,
        path_component: OsString,
    },
    Directory {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        children: Vec<Rc<RefCell<Node>>>,
        path_component: OsString,
        status: ContainerStatus,
    },
    File {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        path_component: OsString,
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

    fn remove_child(&mut self, id: NodeId) {
        let remove = |id: NodeId, children: &mut Vec<Rc<RefCell<Node>>>| {
            if let Some(to_remove) = children
                .iter()
                .enumerate()
                .find(|(_, child)| child.borrow().id() == id)
                .map(|(index, _)| index)
            {
                children.remove(to_remove);
            }
        };

        match self {
            Node::Root { children, .. } => {
                remove(id, children);
            }
            Node::Directory { children, .. } => {
                remove(id, children);
            }
            Node::File { .. } => {}
        };
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

    fn path_component(&self) -> OsString {
        match self {
            Node::Root { path_component, .. } => path_component,
            Node::Directory { path_component, .. } => path_component,
            Node::File { path_component, .. } => path_component,
        }
        .clone()
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

    fn is_directory(&self) -> bool {
        matches!(self, Node::Directory { .. })
    }
}

struct FileExplorerModel {
    root: Rc<RefCell<Node>>,
    index: BTreeMap<NodeId, Rc<RefCell<Node>>>,
    linear_index: Vec<(NodeId, usize)>,
    next_node_id: usize,
    selection: Option<NodeId>,
}

impl FileExplorerModel {
    pub fn new(root_path_component: OsString) -> Self {
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

    pub fn add(&mut self, parent_id: NodeId, entries: Vec<NewEntry>) {
        for new_entry in entries {
            let new_path_component = new_entry.path_component();

            // Check for duplicate
            if let Some(parent_node) = self.get_node(parent_id).cloned() {
                let child_with_path_component = parent_node.borrow().children().find(|child| {
                    let child = self.get_node(*child).unwrap();

                    child.borrow().path_component() == new_path_component
                });

                if child_with_path_component.is_none() {
                    match new_entry {
                        NewEntry::File { path_component } => {
                            self.add_leaf(parent_id, path_component);
                        }
                        NewEntry::Directory { path_component } => {
                            self.add_container(parent_id, path_component);
                        }
                    }
                }
            }
        }

        self.set_status(parent_id, ContainerStatus::Expanded);
    }

    /// Adding a node changes the tree structure so
    /// linear index must be updated using update_linear_index().
    fn add_container(&mut self, parent: NodeId, path_component: OsString) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.get_node(parent).unwrap();
        let mut new_node = Node::Directory {
            id: new_node_id,
            parent: Rc::downgrade(parent_node),
            children: Vec::new(),
            path_component,
            status: ContainerStatus::NotLoaded,
        };

        new_node.set_parent(Rc::downgrade(parent_node));

        let new_node = Rc::new(RefCell::new(new_node));

        parent_node.borrow_mut().add_child(new_node.clone());
        self.index.insert(new_node_id, new_node);

        new_node_id
    }

    /// Adding a node changes the tree structure so
    /// linear index must be updated using update_linear_index().
    fn add_leaf(&mut self, parent: NodeId, path_component: OsString) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.get_node(parent).unwrap();
        let mut new_node = Node::File {
            id: new_node_id,
            parent: Rc::downgrade(parent_node),
            path_component,
        };

        new_node.set_parent(Rc::downgrade(parent_node));

        let new_node = Rc::new(RefCell::new(new_node));

        parent_node.borrow_mut().add_child(new_node.clone());
        self.index.insert(new_node_id, new_node);

        new_node_id
    }

    pub fn remove(&mut self, id: NodeId) {
        if let Some(node) = self.get_node(id) {
            if let Some(parent) = node.borrow().parent() {
                let parent_node = self.get_node(parent).unwrap();

                parent_node.borrow_mut().remove_child(id);
            }

            self.index.remove(&id);
            self.update_linear_index();
        }
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

            let current_node = self.get_node(current).unwrap();

            if matches!(current_node.borrow().status(), ContainerStatus::Expanded) {
                for (index, child_id) in current_node.borrow().children().enumerate() {
                    stack.insert(index, (child_id, current_depth + 1));
                }
            }
        }
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        let node = self.get_node(id)?;

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

    pub fn path_component(&self, id: NodeId) -> Option<OsString> {
        let node = self.get_node(id)?;

        Some(node.borrow().path_component())
    }

    /// Changing the status changes the structure of the tree so
    /// linear index must be updated using update_linear_index().
    pub fn set_status(&mut self, id: NodeId, status: ContainerStatus) {
        let node = self.get_node(id).unwrap();

        node.borrow_mut().set_status(status);
    }

    pub fn status(&self, id: NodeId) -> Option<ContainerStatus> {
        let node = self.get_node(id)?;

        Some(node.borrow().status())
    }

    pub fn expand_collapse(&self, id: NodeId) -> Option<Task<crate::Message>> {
        if let Some(node) = self.get_node(id) {
            if let Node::Directory { status, .. } = node.borrow().deref() {
                match status {
                    ContainerStatus::Expanded => {
                        return Some(Task::done(crate::Message::FileExplorer(Message::Collapse(
                            id,
                        ))))
                    }
                    ContainerStatus::Collapsed => {
                        return Some(Task::done(crate::Message::FileExplorer(Message::Expand(
                            id,
                        ))))
                    }
                    ContainerStatus::NotLoaded => {
                        let path = self.path(id);

                        return Some(Task::perform(
                            load_directory_entries(path),
                            move |entries| {
                                crate::Message::FileExplorer(Message::ChildrenLoaded(id, entries))
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

    fn get_node(&self, id: NodeId) -> Option<&Rc<RefCell<Node>>> {
        self.index.get(&id)
    }

    /// Get the `NodeId` from a `Path`.  
    /// Mirror of `FileExplorer::path()`.
    pub fn node(&self, path_buf: &Path) -> Option<NodeId> {
        let mut path_buf = path_buf.to_path_buf();
        let mut parent_node_id: Option<NodeId> = None;

        while !path_buf.as_os_str().is_empty() {
            match parent_node_id {
                Some(current_node_id) => {
                    if let Some(parent_node) = self.get_node(current_node_id) {
                        if let Some(component_path_to_find) = path_buf
                            .components()
                            .next()
                            .map(|component| component.as_os_str().to_os_string())
                        {
                            let mut have_result = false;
                            for child_id in parent_node.borrow().children() {
                                if let Some(child) = self.get_node(child_id) {
                                    if component_path_to_find == child.borrow().path_component() {
                                        parent_node_id = Some(child_id);
                                        let temp_path_buf = path_buf
                                            .strip_prefix(&component_path_to_find)
                                            .unwrap()
                                            .to_path_buf();

                                        path_buf = temp_path_buf;
                                        have_result = true;
                                        break;
                                    }
                                }
                            }

                            if !have_result {
                                return None;
                            }
                        }
                    }
                }
                None => {
                    let component_path = self
                        .index
                        .get(&self.root_id())
                        .unwrap()
                        .borrow()
                        .path_component();

                    path_buf = path_buf
                        .strip_prefix(&component_path)
                        .unwrap()
                        .to_path_buf();
                    parent_node_id = Some(self.root_id());
                }
            }
        }

        parent_node_id
    }

    pub fn set_selection(&mut self, selection: Option<NodeId>) {
        self.selection = selection;
    }

    pub fn selection(&self) -> Option<NodeId> {
        self.selection
    }

    pub fn is_directory(&self, id: NodeId) -> bool {
        if let Some(node) = self.get_node(id) {
            return node.borrow().is_directory();
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use iced_test::{selector::text, Error};
    use temp_dir_builder::TempDirectoryBuilder;

    use crate::{
        file_explorer::{self, NewEntry, NodeId},
        tests::simulator,
        Message, SEx,
    };

    #[test]
    fn test_load_file() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![NewEntry::File {
                    path_component: "test_sine_L.wav".into(),
                }],
            ),
        ));

        let mut ui = simulator(&app);

        ui.find(text("test_sine_L.wav")).unwrap();

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_load_file")?);

        Ok(())
    }

    #[test]
    fn test_load_tree() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let foo_node_id = NodeId::new(1);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![NewEntry::Directory {
                    path_component: "foo".into(),
                }],
            ),
        ));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                foo_node_id,
                vec![NewEntry::File {
                    path_component: "test_sine_L.wav".into(),
                }],
            ),
        ));

        let mut ui = simulator(&app);

        ui.find(text("foo")).unwrap();
        ui.find(text("test_sine_L.wav")).unwrap();

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_load_tree")?);

        Ok(())
    }

    #[test]
    fn test_collapse() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let foo_node_id = NodeId::new(1);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![
                    NewEntry::Directory {
                        path_component: "foo".into(),
                    },
                    NewEntry::Directory {
                        path_component: "bar".into(),
                    },
                ],
            ),
        ));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                foo_node_id,
                vec![NewEntry::File {
                    path_component: "test_sine_L.wav".into(),
                }],
            ),
        ));
        let _ = app.update(Message::FileExplorer(file_explorer::Message::Collapse(
            foo_node_id,
        )));

        let mut ui = simulator(&app);

        ui.find(text("foo")).unwrap();
        ui.find(text("test_sine_L.wav")).unwrap_err();

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_collapse")?);

        Ok(())
    }

    #[test]
    fn test_select() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![NewEntry::File {
                    path_component: "test_sine_L.wav".into(),
                }],
            ),
        ));
        let _ = app.update(Message::FileExplorer(file_explorer::Message::Select(Some(
            NodeId::new(1),
        ))));

        let mut ui = simulator(&app);

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_image("snapshots/test_select")?);

        Ok(())
    }

    #[test]
    fn test_select_next() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![
                    NewEntry::File {
                        path_component: "test_sine_L.wav".into(),
                    },
                    NewEntry::File {
                        path_component: "test_sine_LR.wav".into(),
                    },
                ],
            ),
        ));
        let _ = app.update(Message::FileExplorer(file_explorer::Message::Select(Some(
            NodeId::new(1),
        ))));
        let _ = app.update(Message::FileExplorer(file_explorer::Message::SelectNext));

        let mut ui = simulator(&app);

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_select_next")?);

        Ok(())
    }

    #[test]
    fn test_select_previous() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![
                    NewEntry::File {
                        path_component: "test_sine_L.wav".into(),
                    },
                    NewEntry::File {
                        path_component: "test_sine_LR.wav".into(),
                    },
                ],
            ),
        ));
        let _ = app.update(Message::FileExplorer(file_explorer::Message::Select(Some(
            NodeId::new(2),
        ))));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::SelectPrevious,
        ));

        let mut ui = simulator(&app);

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_select_previous")?);

        Ok(())
    }

    #[test]
    fn test_removed() -> Result<(), Error> {
        let test_dir = TempDirectoryBuilder::default().build().unwrap();
        let (mut app, _task) = SEx::new();

        let root_node_id = NodeId::new(0);
        let _ = app.update(Message::OpenDirectory(Some(test_dir.path().to_path_buf())));
        let foo_node_id = NodeId::new(1);
        let bar_node_id = NodeId::new(2);
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                root_node_id,
                vec![
                    NewEntry::Directory {
                        path_component: "foo".into(),
                    },
                    NewEntry::Directory {
                        path_component: "bar".into(),
                    },
                ],
            ),
        ));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                foo_node_id,
                vec![NewEntry::File {
                    path_component: "test_sine_L.wav".into(),
                }],
            ),
        ));
        let _ = app.update(Message::FileExplorer(
            file_explorer::Message::ChildrenLoaded(
                bar_node_id,
                vec![NewEntry::File {
                    path_component: "test_sine_R.wav".into(),
                }],
            ),
        ));
        let path_to_remove = test_dir.path().join("foo");
        let _ = app.update(Message::FileExplorer(file_explorer::Message::Removed(
            path_to_remove,
        )));

        let mut ui = simulator(&app);

        ui.find(text("foo")).unwrap_err();
        ui.find(text("bar")).unwrap();
        ui.find(text("test_sine_L.wav")).unwrap_err();
        ui.find(text("test_sine_R.wav")).unwrap();

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_removed")?);

        Ok(())
    }
}
