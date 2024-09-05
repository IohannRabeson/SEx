use std::{
    cell::RefCell, collections::{BTreeMap, VecDeque}, path::PathBuf, rc::{Rc, Weak}, usize
};

use iced::{widget::{row, scrollable, text, Column, MouseArea, Space}, Element, Length};

use crate::Message;

#[derive(Debug, Clone)]
pub enum FileExplorerMessage {
    RequestLoad(NodeId, PathBuf),
    ChildrenLoaded(NodeId, Vec<EntryFound>),
    Collapse(NodeId),
    Expand(NodeId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryFound {
    Directory { path_component: String },
    File { path_component: String },
}

pub struct FileExplorerModel {
    root: Rc<RefCell<Node>>,
    index: BTreeMap<NodeId, Rc<RefCell<Node>>>,
    next_node_id: usize,
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


pub fn view(tree: Option<&FileExplorerModel>) -> Element<Message> {
    const DEPTH_OFFSET: f32 = 16f32;

    let mut main_column = Column::new();

    if let Some(tree) = tree {
        for (id, depth) in tree.dfs_visit() {
            if id == tree.root_id() {
                continue;
            }
            let path_component = tree.path_component(id).unwrap();
            let status = tree.status(id).unwrap();
            let row = row![
                Space::new(Length::Fixed(depth as f32 * DEPTH_OFFSET), Length::Shrink),
                show_children_control(&tree, id, status),
                Space::new(Length::Fixed(5f32), Length::Shrink),
                text(path_component),
            ];

            main_column = main_column.push(row);
        }
    }
    scrollable(main_column).width(Length::Fill).into()
}

fn show_children_control(tree: &FileExplorerModel, id: NodeId, status: ContainerStatus) -> Element<Message> {
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
    Container {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        children: Vec<Rc<RefCell<Node>>>,
        path_component: String,
        status: ContainerStatus,
    },
    Leaf {
        id: NodeId,
        parent: Weak<RefCell<Node>>,
        path_component: String,
    },
}

impl Node {
    fn id(&self) -> NodeId {
        match self {
            Node::Root { id, .. } => *id,
            Node::Container { id, .. } => *id,
            Node::Leaf { id, .. } => *id,
        }
    }

    fn parent(&self) -> Option<NodeId> {
        match self {
            Node::Root { .. } => None,
            Node::Container { parent, .. } => parent.upgrade().map(|node| node.borrow().id()),
            Node::Leaf { parent, .. } => parent.upgrade().map(|node| node.borrow().id()),
        }
    }

    fn set_parent(&mut self, new_parent: Weak<RefCell<Node>>) {
        match self {
            Node::Root { .. } => {
                panic!("Trying to set parent of the root.")
            }
            Node::Container { parent, .. } => {
                *parent = new_parent;
            }
            Node::Leaf { parent, .. } => {
                *parent = new_parent;
            }
        }
    }

    fn add_child(&mut self, child: Rc<RefCell<Node>>) {
        match self {
            Node::Root { children, .. } => {
                children.push(child);
            }
            Node::Container { children, .. } => {
                children.push(child);
            }
            Node::Leaf { .. } => {
                panic!("Trying to add a child to a leaf")
            }
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = NodeId> + '_> {
        match self {
            Node::Root { children, .. } => Box::new(children.iter().map(|node| node.borrow().id())),
            Node::Container { children, .. } => {
                Box::new(children.iter().map(|node| node.borrow().id()))
            }
            Node::Leaf { .. } => Box::new(std::iter::empty::<NodeId>()),
        }
    }

    fn path_component(&self) -> String {
        match self {
            Node::Root { path_component, .. } => path_component.clone(),
            Node::Container { path_component, .. } => path_component.clone(),
            Node::Leaf { path_component, .. } => path_component.clone(),
        }
    }

    fn status(&self) -> ContainerStatus {
        match self {
            Node::Root { .. } => ContainerStatus::Expanded,
            Node::Container { status, .. } => *status,
            Node::Leaf { .. } => ContainerStatus::Empty,
        }
    }

    fn set_status(&mut self, new_status: ContainerStatus) {
        if let Node::Container { status, .. } = self {
            *status = new_status;
        }
    }
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
        }
    }

    pub fn root_id(&self) -> NodeId {
        let root = self.root.borrow();

        if let Node::Root { id, .. } = &*root {
            return *id;
        } else {
            panic!("")
        }
    }

    pub fn add_container(&mut self, parent: NodeId, path_component: String) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.index.get(&parent).unwrap();
        let mut new_node = Node::Container {
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

    pub fn add_leaf(&mut self, parent: NodeId, path_component: String) -> NodeId {
        let new_node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let parent_node = self.index.get(&parent).unwrap();
        let mut new_node = Node::Leaf {
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

    pub fn dfs_visit(&self) -> Vec<(NodeId, usize)> {
        let initial_depth = 0;
        let mut stack = VecDeque::from([(self.root_id(), initial_depth)]);
        let mut results = Vec::new();

        while let Some((current, current_depth)) = stack.pop_front() {
            results.push((current, current_depth));

            let current_node = self.index.get(&current).unwrap();

            if matches!(current_node.borrow().status(), ContainerStatus::Expanded) {
                for (index, child_id) in current_node.borrow().children().enumerate() {
                    stack.insert(index, (child_id, current_depth + 1));
                }
            }
        }

        results
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        let node = self.index.get(&id)?;

        node.borrow().parent()
    }

    pub fn path_component(&self, id: NodeId) -> Option<String> {
        let node = self.index.get(&id)?;

        Some(node.borrow().path_component())
    }

    pub fn set_status(&mut self, id: NodeId, status: ContainerStatus) {
        let node = self.index.get(&id).unwrap();

        node.borrow_mut().set_status(status);
    }

    pub fn status(&self, id: NodeId) -> Option<ContainerStatus> {
        let node = self.index.get(&id)?;

        Some(node.borrow().status())
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
}
