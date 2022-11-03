use slotmap::{new_key_type, SlotMap};

use super::{Dynamic, DynamicItems, DynamicList, Dynamics, Rendered, RenderedListItem};

new_key_type! { pub struct NodeId; }

#[derive(Debug)]
pub struct RenderedBuilder {
    nodes: SlotMap<NodeId, Node>,
    last_node: NodeId,
}

#[derive(Debug)]
struct Node {
    parent: NodeId,
    value: NodeValue,
}

#[derive(Debug)]
enum NodeValue {
    Items(ItemsNode),
    List(ListNode),
    Nested(Rendered),
}

#[derive(Debug, Default)]
struct ItemsNode {
    statics: Vec<String>,
    dynamics: Vec<DynamicNode>,
    templates: Vec<Vec<String>>,
}

#[derive(Debug)]
struct ListNode {
    statics: Vec<String>,
    dynamics: Vec<Vec<DynamicNode>>,
    iteration: usize,
}

#[derive(Debug)]
enum DynamicNode {
    String(String),
    Nested(NodeId),
}

impl RenderedBuilder {
    pub fn new() -> Self {
        let mut nodes = SlotMap::with_key();
        let last_node = nodes.insert(Node::new(
            NodeId::default(),
            NodeValue::Items(ItemsNode::default()),
        ));
        RenderedBuilder { nodes, last_node }
    }

    pub fn build(mut self) -> Rendered {
        let root = self.nodes.remove(self.last_node).unwrap();
        root.build(&mut self)
    }

    pub fn push_nested(&mut self, other: Rendered) {
        let parent = self.parent_of(self.last_node).unwrap();
        let id = self
            .nodes
            .insert(Node::new(parent, NodeValue::Nested(other)));
        let last_node = self.last_node_mut();
        match &mut last_node.value {
            NodeValue::Items(items) => {
                items.statics.push(String::new());
                items.dynamics.push(DynamicNode::Nested(id));
            }
            NodeValue::List(_) => {
                self.nodes.remove(id);
                todo!()
            }
            NodeValue::Nested(_) => {
                self.nodes.remove(id);
                todo!()
            }
        }
    }

    pub fn push_static(&mut self, s: &str) {
        self.last_node_mut().push_static(s)
    }

    pub fn push_dynamic(&mut self, s: String) {
        self.last_node_mut().push_dynamic(s)
    }

    pub fn push_if_frame(&mut self) {
        self.push_dynamic_node(NodeValue::Items(ItemsNode::default()));
    }

    pub fn push_for_frame(&mut self) {
        self.push_dynamic_node(NodeValue::List(ListNode::default()));
    }

    pub fn push_for_item(&mut self) {
        let last_node = self.last_node_mut();
        match &mut last_node.value {
            NodeValue::Items(_) => {
                panic!("push_for_item cannot be called outside the context of a for loop");
            }
            NodeValue::List(list) => {
                list.iteration = list.iteration.wrapping_add(1); // First iteration will be 0
                list.dynamics.push(vec![]);
            }
            NodeValue::Nested(_) => todo!(),
        }
    }

    pub fn pop_for_item(&mut self) {}

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

    fn push_dynamic_node(&mut self, value: NodeValue) {
        let id = self.nodes.insert(Node::new(self.last_node, value));
        let last_node = self.last_node_mut();
        match &mut last_node.value {
            NodeValue::Items(items) => {
                items.dynamics.push(DynamicNode::Nested(id));
            }
            NodeValue::List(list) => match list.dynamics.last_mut() {
                Some(last_list) => last_list.push(DynamicNode::Nested(id)),
                None => list.dynamics.push(vec![DynamicNode::Nested(id)]),
            },
            NodeValue::Nested(_) => todo!(),
        }
        self.last_node = id;
    }
}

impl Default for RenderedBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Node {
    fn new(parent: NodeId, value: NodeValue) -> Self {
        Node { parent, value }
    }

    fn build(self, tree: &mut RenderedBuilder) -> Rendered {
        match self.value {
            NodeValue::Items(items) => items.build(tree),
            NodeValue::List(list) => list.build(tree),
            NodeValue::Nested(nested) => nested,
        }
    }

    fn push_static(&mut self, s: &str) {
        match &mut self.value {
            NodeValue::Items(items) => items.push_static(s),
            NodeValue::List(list) => list.push_static(s),
            NodeValue::Nested(_) => todo!(),
        }
    }

    fn push_dynamic(&mut self, s: String) {
        match &mut self.value {
            NodeValue::Items(items) => items.push_dynamic(s),
            NodeValue::List(list) => list.push_dynamic(s),
            NodeValue::Nested(_) => todo!(),
        }
    }
}

impl ItemsNode {
    fn build(mut self, tree: &mut RenderedBuilder) -> Rendered {
        let dynamics: Vec<_> = self
            .dynamics
            .into_iter()
            .map(|dynamic| dynamic.build_items(tree))
            .collect();

        insert_empty_strings(&mut self.statics, dynamics.len());

        Rendered {
            statics: self.statics,
            dynamics: Dynamics::Items(DynamicItems(dynamics)),
            templates: self.templates,
        }
    }

    fn push_static(&mut self, s: &str) {
        push_or_extend_static_string(&mut self.statics, self.dynamics.len(), s);
    }

    fn push_dynamic(&mut self, s: String) {
        if self.statics.is_empty() {
            self.statics.push(String::new());
        }

        self.dynamics.push(DynamicNode::String(s));
    }
}

impl ListNode {
    fn build(self, tree: &mut RenderedBuilder) -> Rendered {
        let mut templates = vec![];

        let dynamics: Vec<Vec<_>> = self
            .dynamics
            .into_iter()
            .map(|dynamics| {
                dynamics
                    .into_iter()
                    .map(|dynamic| dynamic.build_list(tree, &mut templates))
                    .collect()
            })
            .collect();

        Rendered {
            statics: self.statics,
            dynamics: Dynamics::List(DynamicList(dynamics)),
            templates,
        }
    }

    fn push_static(&mut self, s: &str) {
        if self.iteration == 0 {
            let dynamics_len = self.dynamics.first().map(|first| first.len()).unwrap_or(0);
            push_or_extend_static_string(&mut self.statics, dynamics_len, s);
        }
    }

    fn push_dynamic(&mut self, s: String) {
        self.dynamics
            .last_mut()
            .unwrap()
            .push(DynamicNode::String(s));
    }
}

impl Default for ListNode {
    fn default() -> Self {
        Self {
            statics: Default::default(),
            dynamics: Default::default(),
            iteration: usize::MAX,
        }
    }
}

impl DynamicNode {
    fn build_items(self, tree: &mut RenderedBuilder) -> Dynamic<Rendered> {
        match self {
            DynamicNode::String(s) => Dynamic::String(s),
            DynamicNode::Nested(id) => {
                let mut nested = tree.nodes.remove(id).unwrap().build(tree);
                match nested.dynamics {
                    Dynamics::Items(ref items) => {
                        if nested.statics.is_empty() && items.is_empty() {
                            Dynamic::String(String::new())
                        } else {
                            insert_empty_strings(&mut nested.statics, items.len());
                            Dynamic::Nested(nested)
                        }
                    }
                    Dynamics::List(list) => {
                        let dynamics_len = list.first().map(|first| first.len()).unwrap_or(0);
                        if nested.statics.is_empty() && dynamics_len == 0 {
                            Dynamic::String(String::new())
                        } else {
                            insert_empty_strings(&mut nested.statics, dynamics_len);

                            Dynamic::Nested(Rendered {
                                statics: nested.statics,
                                dynamics: Dynamics::List(list),
                                templates: nested.templates,
                            })
                        }
                    }
                }
            }
        }
    }

    fn build_list(
        self,
        tree: &mut RenderedBuilder,
        templates: &mut Vec<Vec<String>>,
    ) -> Dynamic<RenderedListItem> {
        match self {
            DynamicNode::String(s) => Dynamic::String(s),
            DynamicNode::Nested(id) => {
                let node = tree.nodes.remove(id).unwrap();
                match node.value {
                    NodeValue::Items(mut items) => {
                        if items.statics.is_empty() && items.dynamics.is_empty() {
                            Dynamic::String(String::new())
                        } else {
                            let dynamics: Vec<_> = items
                                .dynamics
                                .into_iter()
                                .map(|dynamic| dynamic.build_list(tree, templates))
                                .collect();

                            insert_empty_strings(&mut items.statics, dynamics.len());
                            let statics = templates
                                .iter()
                                .enumerate()
                                .find_map(|(i, template)| {
                                    if vecs_match(template, &items.statics) {
                                        Some(i)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_else(|| {
                                    templates.push(items.statics);
                                    templates.len() - 1
                                });

                            Dynamic::Nested(RenderedListItem { statics, dynamics })
                        }
                    }
                    NodeValue::List(_) => todo!(),
                    NodeValue::Nested(_) => todo!(),
                }
            }
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

fn push_or_extend_static_string(statics: &mut Vec<String>, dynamics_len: usize, s: &str) {
    // If statics length is >= dynamics length, we should extend the previous static
    // string.
    let statics_len = statics.len();
    match statics.last_mut() {
        Some(static_string) if statics_len > dynamics_len => static_string.push_str(s),
        _ => statics.push(s.to_string()),
    }
}

fn vecs_match<T: PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
    let matching = a.iter().zip(b.iter()).filter(|&(a, b)| a == b).count();
    matching == a.len() && matching == b.len()
}
