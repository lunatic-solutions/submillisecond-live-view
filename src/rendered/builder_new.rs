use slotmap::{new_key_type, SlotMap};

use super::{DynamicItems, Dynamics, Rendered};

new_key_type! { pub struct NodeId; }

#[derive(Debug)]
pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    last_node: NodeId,
}

#[derive(Debug)]
pub struct Node {
    parent: NodeId,
    value: NodeValue,
}

#[derive(Debug)]
pub enum NodeValue {
    Items(ItemsNode),
    List(ListNode),
}

#[derive(Debug, Default)]
pub struct ItemsNode {
    statics: Vec<String>,
    dynamics: Vec<Dynamic>,
    templates: Vec<Vec<String>>,
}

#[derive(Debug, Default)]
pub struct ListNode {
    statics: Vec<String>,
    dynamics: Vec<Dynamic>,
    templates: Vec<Vec<String>>,
}

#[derive(Debug)]
pub enum Dynamic {
    String(String),
    Items(NodeId),
    List(NodeId),
}

impl Tree {
    pub fn new() -> Self {
        let mut nodes = SlotMap::with_key();
        let last_node = nodes.insert(Node::new(
            NodeId::default(),
            NodeValue::Items(ItemsNode::default()),
        ));
        Tree { nodes, last_node }
    }

    pub fn build(mut self) -> Rendered {
        let root = self.nodes.remove(self.last_node).unwrap();
        root.build(&mut self)
    }

    pub fn push_static(&mut self, s: &str) {
        self.last_node_mut().push_static(s)
    }

    pub fn push_dynamic(&mut self, s: String) {
        self.last_node_mut().push_dynamic(s)
    }

    pub fn push_if_frame(&mut self) {
        self.push_dynamic_node(NodeValue::Items(ItemsNode::default()), Dynamic::Items);
    }

    pub fn push_for_frame(&mut self) {
        self.push_dynamic_node(NodeValue::List(ListNode::default()), Dynamic::List);
    }

    pub fn pop_frame(&mut self) {
        if let Some(parent_id) = self.parent_of(self.last_node) {
            self.last_node = parent_id;
        }
    }

    fn last_node_mut(&mut self) -> &mut Node {
        self.nodes.get_mut(self.last_node).unwrap()
    }

    fn parent_of(&mut self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id).map(|node| node.parent)
    }

    fn push_dynamic_node(&mut self, value: NodeValue, f: impl Fn(NodeId) -> Dynamic) {
        let id = self.nodes.insert(Node::new(self.last_node, value));
        let last_node = self.last_node_mut();
        match &mut last_node.value {
            NodeValue::Items(items) => {
                items.dynamics.push(f(id));
            }
            NodeValue::List(list) => {
                list.dynamics.push(f(id));
            }
        }
        self.last_node = id;
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Node {
    fn new(parent: NodeId, value: NodeValue) -> Self {
        Node { parent, value }
    }

    fn build(self, tree: &mut Tree) -> Rendered {
        println!("{:?}", self);
        match self.value {
            NodeValue::Items(items) => items.build(tree),
            NodeValue::List(_) => todo!(),
        }
    }

    fn push_static(&mut self, s: &str) {
        match &mut self.value {
            NodeValue::Items(items) => items.push_static(s),
            NodeValue::List(list) => list.push_static(s),
        }
    }

    fn push_dynamic(&mut self, s: String) {
        match &mut self.value {
            NodeValue::Items(items) => items.push_dynamic(s),
            NodeValue::List(list) => list.push_dynamic(s),
        }
    }
}

impl ItemsNode {
    fn build(mut self, tree: &mut Tree) -> Rendered {
        let dynamics: Vec<_> = self
            .dynamics
            .into_iter()
            .map(|dynamic| dynamic.build(tree))
            .collect();

        insert_empty_strings(&mut self.statics, dynamics.len());

        Rendered {
            statics: self.statics,
            dynamics: Dynamics::Items(DynamicItems(dynamics)),
            templates: self.templates,
        }
    }

    fn push_static(&mut self, s: &str) {
        // If statics length is >= dynamics length, we should extend the previous static
        // string.
        let statics_len = self.statics.len();
        let dynamics_len = self.dynamics.len();
        match self.statics.last_mut() {
            Some(static_string) if statics_len > dynamics_len => static_string.push_str(s),
            _ => self.statics.push(s.to_string()),
        }
    }

    fn push_dynamic(&mut self, s: String) {
        self.dynamics.push(Dynamic::String(s));
    }
}

impl ListNode {
    fn push_static(&mut self, s: &str) {
        todo!()
    }

    fn push_dynamic(&mut self, s: String) {
        todo!()
    }
}

impl Dynamic {
    fn build(self, tree: &mut Tree) -> super::Dynamic<Rendered> {
        println!("{:?}", self);
        match self {
            Dynamic::String(s) => super::Dynamic::String(s),
            Dynamic::Items(items) => {
                let mut nested = tree.nodes.remove(items).unwrap().build(tree);
                match &nested.dynamics {
                    Dynamics::Items(items) => {
                        if nested.statics.is_empty() && items.is_empty() {
                            super::Dynamic::String(String::new())
                        } else {
                            insert_empty_strings(&mut nested.statics, items.len());
                            super::Dynamic::Nested(nested)
                        }
                    }
                    Dynamics::List(_) => todo!(),
                }
            }
            Dynamic::List(_) => todo!(),
        }
    }
}

fn insert_empty_strings(statics: &mut Vec<String>, dynamics_len: usize) {
    if dynamics_len > 0 {
        let missing_empty_string_count = dynamics_len + 1 - statics.len();
        for _ in 0..missing_empty_string_count {
            statics.push(String::new());
        }
    }
}
