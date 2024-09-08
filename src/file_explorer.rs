use std::{
    cell::RefCell, collections::{BTreeMap, VecDeque}, ops::{Deref, DerefMut}, path::PathBuf, rc::{Rc, Weak}, usize
};

use iced::{
    alignment::Vertical, widget::{container, row, scrollable, svg, text, Column, MouseArea, Space}, Element, Length, Padding, Task, Theme
};

use crate::{load_directory_entries, Message};

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
    Directory { path_component: String },
    File { path_component: String },
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

fn selected_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(theme.palette().primary)),
        ..Default::default()
    }
}

pub fn view(tree: Option<&FileExplorerModel>) -> Element<Message> {
    const DEPTH_OFFSET: f32 = 16f32;

    let mut main_column = Column::new();

    if let Some(tree) = tree {
        for (id, depth) in tree.linear_visit() {
            if id == &tree.root_id() {
                continue;
            }
            let status = tree.status(*id).unwrap();
            let selectable_part = make_selectable_part(
                &tree,
                *id,
                tree.folder_closed_icon.clone(),
                tree.folder_open_icon.clone(),
            );

            let row = row![
                Space::new(Length::Fixed(*depth as f32 * DEPTH_OFFSET), Length::Shrink),
                show_children_control(&tree, *id, status),
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

fn make_selectable_part<'a>(
    model: &'a FileExplorerModel,
    id: NodeId,
    folder: svg::Handle,
    folder_open: svg::Handle,
) -> Element<'a, Message> {
    const FONT_SIZE: u16 = 14;

    let text = |path_component|text(path_component).size(FONT_SIZE);

    let node = &*model.index.get(&id).unwrap().borrow();
    let selectable_part: Element<Message> = match node {
        Node::Root { path_component, .. } => text(path_component.clone()).into(),
        Node::Directory {
            path_component,
            status,
            ..
        } => {
            const SVG_ICON_SIZE: f32 = 20f32;

            let svg = svg(match status {
                ContainerStatus::NotLoaded
                | ContainerStatus::Collapsed
                | ContainerStatus::Empty => folder,
                ContainerStatus::Expanded => folder_open,
            })
            .width(Length::Fixed(SVG_ICON_SIZE))
            .height(Length::Fixed(SVG_ICON_SIZE))
            .style(|theme: &Theme, _status| svg::Style {
                color: Some(theme.palette().text),
            });

            let svg = container(svg).padding(Padding::from([0, 4]));

            row![svg, text(path_component.clone())].align_y(Vertical::Center).into()
        }
        Node::File { path_component, .. } => text(path_component.clone()).into(),
    };

    let mut selectable_part = container(selectable_part);

    if model.selection.is_some_and(|selection| selection == id) {
        selectable_part = selectable_part.style(selected_style);
    }

    MouseArea::new(selectable_part)
        .on_press(Message::FileExplorer(FileExplorerMessage::Select(Some(id))))
        .into()
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
    },
    File {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        path_component: String,
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
    folder_open_icon: svg::Handle,
    folder_closed_icon: svg::Handle,
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
            folder_closed_icon: svg::Handle::from_memory(include_bytes!(
                "../icons/folder-svgrepo-com.svg"
            )),
            folder_open_icon: svg::Handle::from_memory(include_bytes!(
                "../icons/folder-open-side-svgrepo-com.svg"
            )),
            linear_index: Vec::new(),
        }
    }

    pub fn root_id(&self) -> NodeId {
        let root = self.root.borrow();

        if let Node::Root { id, .. } = &*root {
            return *id;
        } else {
            panic!("The root node is not a Root")
        }
    }

    pub fn add(&mut self, parent_id: NodeId, entries: Vec<NewEntry>) {
        for new_entry in entries {
            match new_entry {
                NewEntry::File { path_component } => {
                    self.add_leaf(parent_id, path_component);
                }
                NewEntry::Directory { path_component } => {
                    self.add_container(parent_id, path_component);
                }
            }
        }

        self.set_status(parent_id, ContainerStatus::Expanded);
    }

    /// Adding a node changes the tree structure so
    /// linear index must be updated using update_linear_index().
    fn add_container(&mut self, parent: NodeId, path_component: String) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.index.get(&parent).unwrap();
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
    fn add_leaf(&mut self, parent: NodeId, path_component: String) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.index.get(&parent).unwrap();
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
        let (index, _) = self.linear_index.iter().enumerate().find(|(_index, (node_id, _))|node_id == &id)?;
        
        self.linear_index.get(index + 1).map(|(id, _)|*id)
    }

    pub fn previous(&self, id: NodeId) -> Option<NodeId> {
        let (index, _) = self.linear_index.iter().enumerate().find(|(_index, (node_id, _))|node_id == &id)?;
        
        if index == 0 {
            return None
        }

        self.linear_index.get(index - 1).map(|(id, _)|*id)
    }

    pub fn path_component(&self, id: NodeId) -> Option<String> {
        let node = self.index.get(&id)?;

        Some(node.borrow().path_component())
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

    pub fn expand_collapse(&mut self, id: NodeId) -> Option<Task<Message>> {
        if let Some(node) = self.index.get(&id) {
            // HACK: I create the path here BEFORE I borrow the node mutably because
            // path also borrows the node. I think I should maybe not have the whole Node
            // possibly mutable (not using RefCell<Node> but just Node). What needs to be mutable 
            // are the fields children, and the status of Node::Directory.
            let path = self.path(id);

            if let Node::Directory { status, .. } = node.borrow_mut().deref_mut(){
                match status {
                    ContainerStatus::Expanded => *status = ContainerStatus::Collapsed,
                    ContainerStatus::Collapsed => *status = ContainerStatus::Expanded,
                    ContainerStatus::NotLoaded => {
                        
                        
                        return Some(Task::perform(load_directory_entries(path), move |entries| {
                            Message::FileExplorer(FileExplorerMessage::ChildrenLoaded(id, entries))
                        }))
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
