//! Builder to build [`Rendered`], used by the `html!` macro.

use slotmap::{new_key_type, SlotMap};

use super::dynamic::DynamicList;
use super::{Dynamic, DynamicItems, Dynamics, Rendered, RenderedListItem};

new_key_type! { struct NodeId; }

/// Rendered builder, used by the `html!` macro.
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
    /// Creates a new [`RenderedBuilder`].
    pub fn new() -> Self {
        let mut nodes = SlotMap::with_key();
        let last_node = nodes.insert(Node::new(
            NodeId::default(),
            NodeValue::Items(ItemsNode::default()),
        ));
        RenderedBuilder { nodes, last_node }
    }

    /// Builds into a [`Rendered`].
    pub fn build(mut self) -> Rendered {
        let root = self.nodes.remove(self.last_node).unwrap();
        root.build(&mut self)
    }

    /// Pushes a [`Rendered`] to be nested.
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

    /// Pushes a static string.
    pub fn push_static(&mut self, s: &str) {
        self.last_node_mut().push_static(s)
    }

    /// Pushes a dynamic string.
    pub fn push_dynamic(&mut self, s: String) {
        self.last_node_mut().push_dynamic(s)
    }

    /// Pushes an if frame.
    pub fn push_if_frame(&mut self) {
        self.push_dynamic_node(NodeValue::Items(ItemsNode::default()));
    }

    /// Pushes a for loop frame.
    pub fn push_for_frame(&mut self) {
        self.push_dynamic_node(NodeValue::List(ListNode::default()));
    }

    /// Pushes an item frame in a for loop.
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

    /// Pops an item from the for loop.
    pub fn pop_for_item(&mut self) {}

    /// Pops a frame.
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
                items.statics.push(String::new());
            }
            NodeValue::List(list) => match list.dynamics.last_mut() {
                Some(last_list) => last_list.push(DynamicNode::Nested(id)),
                None => {
                    list.dynamics.push(vec![DynamicNode::Nested(id)]);
                    list.statics.push(String::new());
                }
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

                            Dynamic::Nested(RenderedListItem {
                                statics,
                                dynamics: vec![Dynamics::List(DynamicList(vec![dynamics]))],
                            })
                        }
                    }
                    NodeValue::List(list) => {
                        let mut longest_dynamic = 0;
                        let dynamics: Vec<_> = list
                            .dynamics
                            .into_iter()
                            .map(|dynamics| {
                                let dynamics: Vec<_> = dynamics
                                    .into_iter()
                                    .map(|dynamic| dynamic.build_list(tree, templates))
                                    .collect();
                                longest_dynamic = longest_dynamic.max(dynamics.len());
                                Dynamics::List(DynamicList(vec![dynamics]))
                            })
                            .collect();

                        let statics = templates
                            .iter()
                            .enumerate()
                            .find_map(|(i, template)| {
                                if vecs_match(template, &list.statics) {
                                    Some(i)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| {
                                templates.push(list.statics);
                                templates.len() - 1
                            });
                        insert_empty_strings(templates.last_mut().unwrap(), longest_dynamic);

                        Dynamic::Nested(RenderedListItem { statics, dynamics })
                    }
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::maud::DOCTYPE;
    use crate::rendered::dynamic::{Dynamic, DynamicItems, DynamicList, Dynamics};
    use crate::rendered::{Rendered, RenderedListItem};
    use crate::{self as submillisecond_live_view, html};

    #[lunatic::test]
    fn basic() {
        let rendered = html! {
            p { "Hello, world!" }
        };

        assert_eq!(rendered.statics, ["<p>Hello, world!</p>"]);
        assert_eq!(rendered.dynamics, Dynamics::Items(DynamicItems(vec![])));
        assert!(rendered.templates.is_empty());
    }

    #[lunatic::test]
    fn dynamic() {
        let rendered = html! {
            (DOCTYPE)
            a href={ ("hey") "/lambda-fairy/maud" } {
                "Hello, world!"
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec![
                    "".to_string(),
                    "<a href=\"".to_string(),
                    "/lambda-fairy/maud\">Hello, world!</a>".to_string()
                ],
                dynamics: Dynamics::Items(DynamicItems(vec![
                    Dynamic::String("<!DOCTYPE html>".to_string()),
                    Dynamic::String("hey".to_string())
                ])),
                templates: vec![]
            }
        );
    }

    #[lunatic::test]
    fn if_statement_false() {
        let logged_in = false;
        let rendered = html! {
            "Welcome "
            @if logged_in {
                "person"
            }
            "."
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["Welcome ".to_string(), ".".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
                templates: vec![]
            }
        );

        let logged_in = false;
        let rendered = html! {
            "Welcome "
            @if logged_in {
                (logged_in.to_string())
            }
            "."
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["Welcome ".to_string(), ".".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
                templates: vec![]
            }
        );
    }

    #[lunatic::test]
    fn if_statement_true() {
        let logged_in = true;
        let rendered = html! {
            "Welcome "
            @if logged_in {
                "person"
            }
            "."
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["Welcome ".to_string(), ".".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["person".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );

        let logged_in = true;
        let rendered = html! {
            "Welcome "
            @if logged_in {
                (logged_in.to_string())
            }
            "."
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["Welcome ".to_string(), ".".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["".to_string(), "".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String(
                        "true".to_string()
                    )])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn if_statement_let_some() {
        let user = Some("Bob");
        let rendered = html! {
            "Welcome "
            @if let Some(user) = user {
                (user)
            } @else {
                "stranger"
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["Welcome ".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["".to_string(), "".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String(
                        "Bob".to_string()
                    )])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn if_statement_let_none() {
        let user: Option<&str> = None;
        let rendered = html! {
            "Welcome "
            @if let Some(user) = user {
                (user)
            } @else {
                "stranger"
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["Welcome ".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["stranger".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn if_statement_nested() {
        let render = |count: usize| {
            html! {
                @if count >= 1 {
                    p { "Count is high" }
                    @if count >= 2 {
                        p { "Count is very high!" }
                    }
                }
            }
        };

        let rendered = render(0);

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
                templates: vec![],
            }
        );

        let rendered = render(1);

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );

        let rendered = render(2);

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                        statics: vec!["<p>Count is very high!</p>".to_string()],
                        dynamics: Dynamics::Items(DynamicItems(vec![])),
                        templates: vec![],
                    })])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_empty() {
        #[allow(clippy::reversed_empty_ranges)]
        let rendered = html! {
            span { "Hello" }
            @for _ in 0..0 {
                span { "Hi!" }
            }
            span { "world" }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec![
                    "<span>Hello</span>".to_string(),
                    "<span>world</span>".to_string()
                ],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_statics() {
        let rendered = html! {
            @for _ in 0..3 {
                span { "Hi!" }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["<span>Hi!</span>".to_string()],
                    dynamics: Dynamics::List(DynamicList(vec![vec![], vec![], vec![]])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_dynamics() {
        let names = ["John", "Joe", "Jim"];
        let rendered = html! {
            @for name in names {
                span { (name) }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["<span>".to_string(), "</span>".to_string()],
                    dynamics: Dynamics::List(DynamicList(vec![
                        vec![Dynamic::String("John".to_string())],
                        vec![Dynamic::String("Joe".to_string())],
                        vec![Dynamic::String("Jim".to_string())],
                    ])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );

        let names = ["John", "Joe", "Jim"];
        let rendered = html! {
            @for name in names {
                span class=(name) { (name) }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec![
                        "<span class=\"".to_string(),
                        "\">".to_string(),
                        "</span>".to_string()
                    ],
                    dynamics: Dynamics::List(DynamicList(vec![
                        vec![
                            Dynamic::String("John".to_string()),
                            Dynamic::String("John".to_string())
                        ],
                        vec![
                            Dynamic::String("Joe".to_string()),
                            Dynamic::String("Joe".to_string())
                        ],
                        vec![
                            Dynamic::String("Jim".to_string()),
                            Dynamic::String("Jim".to_string())
                        ],
                    ])),
                    templates: vec![],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_multiple() {
        #[allow(clippy::reversed_empty_ranges)]
        let rendered = html! {
            span { "Hello" }
            @for _ in 0..2 {
                span { "A" }
            }
            @for _ in 0..0 {
                span { "B" }
            }
            span { "world" }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec![
                    "<span>Hello</span>".to_string(),
                    "".to_string(),
                    "<span>world</span>".to_string()
                ],
                dynamics: Dynamics::Items(DynamicItems(vec![
                    Dynamic::Nested(Rendered {
                        statics: vec!["<span>A</span>".to_string()],
                        dynamics: Dynamics::List(DynamicList(vec![vec![], vec![]])),
                        templates: vec![]
                    }),
                    Dynamic::String("".to_string()),
                ])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_nested() {
        let a = "Hello";
        let b = "World";
        let rendered = html! {
            @for foo in [[a, b]] {
                @for bar in foo {
                    span { (bar) }
                }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["".to_string(), "".to_string()],
                    dynamics: Dynamics::List(DynamicList(vec![vec![Dynamic::Nested(
                        RenderedListItem {
                            statics: 0,
                            dynamics: vec![
                                Dynamics::List(DynamicList(vec![vec![Dynamic::String(
                                    "Hello".to_string()
                                )]])),
                                Dynamics::List(DynamicList(vec![vec![Dynamic::String(
                                    "World".to_string()
                                )]]))
                            ],
                        },
                    )]])),
                    templates: vec![vec!["<span>".to_string(), "</span>".to_string()]],
                })])),
                templates: vec![],
            }
        );

        let rendered = html! {
            @for foo in [[a, b]] {
                @for bar in foo {
                    span { (bar) }
                    @if bar == "World" {
                        div { "!!!" }
                    }
                }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["".to_string(), "".to_string()],
                    dynamics: Dynamics::List(DynamicList(vec![vec![Dynamic::Nested(
                        RenderedListItem {
                            statics: 1,
                            dynamics: vec![
                                Dynamics::List(DynamicList(vec![vec![
                                    Dynamic::String("Hello".to_string()),
                                    Dynamic::String("".to_string())
                                ]])),
                                Dynamics::List(DynamicList(vec![vec![
                                    Dynamic::String("World".to_string()),
                                    Dynamic::Nested(RenderedListItem {
                                        statics: 0,
                                        dynamics: vec![Dynamics::List(DynamicList(vec![vec![]]))],
                                    })
                                ]]))
                            ],
                        },
                    )]])),
                    templates: vec![
                        vec!["<div>!!!</div>".to_string()],
                        vec!["<span>".to_string(), "</span>".to_string(), "".to_string()]
                    ],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_with_if() {
        let names = ["John", "Joe", "Jim"];
        let rendered = html! {
            @for name in names {
                span { "Welcome, " (name) "." }
                @if name == "Jim" {
                    span { "You are a VIP, " (name.to_lowercase()) }
                }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec![
                        "<span>Welcome, ".to_string(),
                        ".</span>".to_string(),
                        "".to_string()
                    ],
                    dynamics: Dynamics::List(DynamicList(vec![
                        vec![
                            Dynamic::String("John".to_string()),
                            Dynamic::String("".to_string()),
                        ],
                        vec![
                            Dynamic::String("Joe".to_string()),
                            Dynamic::String("".to_string()),
                        ],
                        vec![
                            Dynamic::String("Jim".to_string()),
                            Dynamic::Nested(RenderedListItem {
                                statics: 0,
                                dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                    Dynamic::String("jim".to_string())
                                ]]))],
                            })
                        ],
                    ])),
                    templates: vec![vec![
                        "<span>You are a VIP, ".to_string(),
                        "</span>".to_string()
                    ]],
                })])),
                templates: vec![]
            }
        );
    }

    #[lunatic::test]
    fn for_loop_with_multiple_ifs() {
        let names = ["John", "Joe", "Jim"];
        let rendered = html! {
            @for name in names {
                span { "Welcome, " (name) "." }
                @if name == "Jim" {
                    span { "You are a VIP, " (name.to_lowercase()) }
                    @if name.ends_with('m') {
                        span { (name) " ends with m" }
                    }
                }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec![
                        "<span>Welcome, ".to_string(),
                        ".</span>".to_string(),
                        "".to_string()
                    ],
                    dynamics: Dynamics::List(DynamicList(vec![
                        vec![
                            Dynamic::String("John".to_string()),
                            Dynamic::String("".to_string()),
                        ],
                        vec![
                            Dynamic::String("Joe".to_string()),
                            Dynamic::String("".to_string()),
                        ],
                        vec![
                            Dynamic::String("Jim".to_string()),
                            Dynamic::Nested(RenderedListItem {
                                statics: 1,
                                dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                    Dynamic::String("jim".to_string()),
                                    Dynamic::Nested(RenderedListItem {
                                        statics: 0,
                                        dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                            Dynamic::String("Jim".to_string())
                                        ]]))],
                                    })
                                ]])),],
                            })
                        ],
                    ])),
                    templates: vec![
                        vec!["<span>".to_string(), " ends with m</span>".to_string()],
                        vec![
                            "<span>You are a VIP, ".to_string(),
                            "</span>".to_string(),
                            "".to_string()
                        ],
                    ],
                })])),
                templates: vec![],
            }
        );
    }

    #[lunatic::test]
    fn for_loop_with_many_ifs() {
        let names = ["John", "Joe", "Jim"];
        let rendered = html! {
            @for name in names {
                span { "Welcome, " (name) "." }
                @if name == "Jim" || name == "Joe" {
                    span { "You are a VIP, " (name.to_lowercase()) }
                    @if name.ends_with('m') || name.ends_with('e') {
                        span { (name) " ends with m or e" }
                    }
                }
            }
        };

        assert_eq!(
            rendered,
            Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec![
                        "<span>Welcome, ".to_string(),
                        ".</span>".to_string(),
                        "".to_string()
                    ],
                    dynamics: Dynamics::List(DynamicList(vec![
                        vec![
                            Dynamic::String("John".to_string()),
                            Dynamic::String("".to_string()),
                        ],
                        vec![
                            Dynamic::String("Joe".to_string()),
                            Dynamic::Nested(RenderedListItem {
                                statics: 1,
                                dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                    Dynamic::String("joe".to_string()),
                                    Dynamic::Nested(RenderedListItem {
                                        statics: 0,
                                        dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                            Dynamic::String("Joe".to_string())
                                        ]]))],
                                    })
                                ]]))],
                            }),
                        ],
                        vec![
                            Dynamic::String("Jim".to_string()),
                            Dynamic::Nested(RenderedListItem {
                                statics: 1,
                                dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                    Dynamic::String("jim".to_string()),
                                    Dynamic::Nested(RenderedListItem {
                                        statics: 0,
                                        dynamics: vec![Dynamics::List(DynamicList(vec![vec![
                                            Dynamic::String("Jim".to_string())
                                        ]]))],
                                    })
                                ]]))],
                            }),
                        ],
                    ])),
                    templates: vec![
                        vec!["<span>".to_string(), " ends with m or e</span>".to_string()],
                        vec![
                            "<span>You are a VIP, ".to_string(),
                            "</span>".to_string(),
                            "".to_string()
                        ],
                    ],
                })])),
                templates: vec![]
            }
        );
    }
}
