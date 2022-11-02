use super::{Dynamic, DynamicItems, DynamicList, Dynamics, Rendered, RenderedListItem};

#[derive(Debug)]
pub struct RenderedBuilder {
    statics: Vec<String>,
    dynamics: Dynamics<Self, RenderedListItemBuilder>,
    templates: Vec<Vec<String>>,
    nested: bool,
    loop_count: usize,
}

#[derive(Debug)]
pub struct RenderedListItemBuilder {
    statics: usize,
    dynamics: Vec<Dynamic<Self>>,
    nested: bool,
    level: usize,
}

#[derive(Debug)]
pub enum LastItem<'a> {
    Items(&'a mut RenderedBuilder),
    List(&'a mut RenderedListItemBuilder),
}

impl LastItem<'_> {
    fn as_ptr(&mut self) -> LastItemPtr {
        match self {
            LastItem::Items(items) => LastItemPtr::Items(*items),
            LastItem::List(list) => LastItemPtr::List(*list),
        }
    }
}

enum LastItemPtr {
    Items(*mut RenderedBuilder),
    List(*mut RenderedListItemBuilder),
}

impl LastItemPtr {
    unsafe fn deref<'a>(self) -> LastItem<'a> {
        match self {
            LastItemPtr::Items(items) => LastItem::Items(&mut *items),
            LastItemPtr::List(list) => LastItem::List(&mut *list),
        }
    }
}

pub trait BuildRendered {
    fn push_nested(&mut self, nested: Rendered);
    fn push_static(&mut self, s: &str);
    fn push_dynamic(&mut self, s: String);
    fn push_if_frame(&mut self);
    fn push_for_frame(&mut self);
    fn pop_frame(&mut self);
}

impl RenderedBuilder {
    pub(crate) fn new() -> Self {
        RenderedBuilder {
            statics: vec![],
            dynamics: Dynamics::Items(DynamicItems(vec![])),
            templates: vec![],
            nested: false,
            loop_count: 0,
        }
    }

    pub fn build(self) -> Rendered {
        let dynamics = match self.dynamics {
            Dynamics::Items(items) => Dynamics::Items(DynamicItems(
                items.0.into_iter().map(Dynamic::from).collect(),
            )),
            Dynamics::List(list) => Dynamics::List(DynamicList(
                list.0
                    .into_iter()
                    .map(|list| list.into_iter().map(Dynamic::from).collect())
                    .collect(),
            )),
        };

        Rendered {
            statics: self.statics,
            dynamics,
            templates: self.templates,
        }
    }

    fn next(&mut self) -> Option<LastItem> {
        match &mut self.dynamics {
            Dynamics::Items(inner_items) => inner_items.last_mut().and_then(|last| match last {
                Dynamic::String(_) => None,
                Dynamic::Nested(nested) => Some(LastItem::Items(nested)),
            }),
            Dynamics::List(list) => {
                list.last_mut()
                    .and_then(|last| last.last_mut())
                    .and_then(|last| match last {
                        Dynamic::String(_) => None,
                        Dynamic::Nested(nested) => Some(LastItem::List(nested)),
                    })
            }
        }
    }

    pub fn last_mut(&mut self) -> LastItem {
        enum LastItemPtr {
            Items(*mut RenderedBuilder),
            List(*mut RenderedListItemBuilder),
        }

        let mut current = LastItemPtr::Items(self as *mut Self);

        loop {
            // SAFETY: Rust doesn't like this, though it is safe in this case.
            // This works in polonius, but not Rust's default borrow checker.
            unsafe {
                match current {
                    LastItemPtr::Items(items) => {
                        if !(*items).nested {
                            return LastItem::Items(&mut *items);
                        }

                        match &mut (*items).dynamics {
                            Dynamics::Items(inner_items) => {
                                let next = inner_items.last_mut().and_then(|last| match last {
                                    Dynamic::String(_) => None,
                                    Dynamic::Nested(nested) => Some(nested),
                                });

                                match next {
                                    Some(next) => {
                                        current = LastItemPtr::Items(next);
                                    }
                                    None => {
                                        return LastItem::Items(&mut *items);
                                    }
                                }
                            }
                            Dynamics::List(list) => {
                                let next =
                                    list.0.last_mut().and_then(|last| last.last_mut()).and_then(
                                        |last| match last {
                                            Dynamic::String(_) => None,
                                            Dynamic::Nested(nested) => Some(nested),
                                        },
                                    );

                                match next {
                                    Some(next) => {
                                        current = LastItemPtr::List(next);
                                    }
                                    None => {
                                        return LastItem::Items(&mut *items);
                                    }
                                }
                            }
                        };
                    }
                    LastItemPtr::List(list) => {
                        if !(*list).nested {
                            return LastItem::List(&mut *list);
                        }

                        let next = (*list).dynamics.last_mut().and_then(|last| match last {
                            Dynamic::String(_) => None,
                            Dynamic::Nested(nested) => Some(nested),
                        });

                        match next {
                            Some(next) => {
                                current = LastItemPtr::List(next);
                            }
                            None => {
                                return LastItem::List(&mut *list);
                            }
                        }
                    }
                }
            }
        }
    }

    fn last_items_mut(&mut self) -> &mut Self {
        let mut current = self as *mut Self;

        loop {
            // SAFETY: Rust doesn't like this, though it is safe in this case.
            // This works in polonius, but not Rust's default borrow checker.
            unsafe {
                if !(*current).nested {
                    return &mut *current;
                }

                let next = match &mut (*current).dynamics {
                    Dynamics::Items(items) => items.last_mut().and_then(|last| match last {
                        Dynamic::String(_) => None,
                        Dynamic::Nested(nested) => Some(nested),
                    }),
                    Dynamics::List(_) => None,
                };
                match next {
                    Some(next) => {
                        current = next;
                    }
                    None => {
                        return &mut *current;
                    }
                }
            }
        }
    }

    fn last_parent_mut(&mut self) -> Option<LastItem> {
        if !self.nested {
            return None;
        }

        let mut current = LastItemPtr::Items(self as *mut Self);

        loop {
            unsafe {
                let next = match current {
                    LastItemPtr::Items(items) => (*items).next(),
                    LastItemPtr::List(list) => (*list).next().map(LastItem::List),
                };

                match next {
                    Some(LastItem::Items(RenderedBuilder { nested: false, .. }))
                    | Some(LastItem::List(RenderedListItemBuilder { nested: false, .. }))
                    | None => {
                        return Some(current.deref());
                    }
                    Some(mut next) => {
                        current = next.as_ptr();
                    }
                }
            }
        }
    }
}

impl RenderedBuilder {
    pub fn push_nested(&mut self, _nested: Rendered) {
        // let last = self.last_mut();
        // let nested: RenderedBuilder = nested.into();
        // let mut statics = nested.statics.into_iter();
        // if let Some(first_static) = statics.next() {
        //     match last.statics.last_mut() {
        //         Some(static_string) => static_string.push_str(&first_static),
        //         None => last.statics.push(first_static),
        //     }
        //     last.statics.extend(statics);
        // }
        // last.dynamics.extend(nested.dynamics);
    }

    pub fn push_static(&mut self, s: &str) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => match &mut last.dynamics {
                Dynamics::Items(items) => {
                    if last.nested && items.is_empty() {
                        println!("Push A: {s}");
                        items.push(Dynamic::Nested(RenderedBuilder {
                            statics: vec![s.to_string()],
                            dynamics: Dynamics::Items(DynamicItems(vec![])),
                            templates: vec![],
                            nested: false,
                            loop_count: 0,
                        }));
                    } else if last.statics.len() >= items.len() {
                        println!("Push B: {s}");
                        match last.statics.last_mut() {
                            Some(static_string) => static_string.push_str(s),
                            None => last.statics.push(s.to_string()),
                        }
                    } else {
                        println!("Push C: {s}");
                        last.statics.push(s.to_string());
                    }
                }
                Dynamics::List(list) => {
                    if last.nested {
                        last.templates.insert(0, vec![s.to_string()]);
                    } else if last.loop_count < 1 {
                        last.statics.push(s.to_string());
                    }
                }
            },
            LastItem::List(last) => {
                // TODO Last static
                let last_items = self.last_items_mut();
                // if last_items.len() < last
                // dbg!(last_items.templates.len(), )
                match last_items.templates.first_mut() {
                    Some(last_item) => {
                        // if last_item.len() >= last_dynamics_len {
                        //     match last_item.last_mut() {
                        //         Some(static_string) => static_string.push_str(s),
                        //         None => last_item.push(s.to_string()),
                        //     }
                        // } else {
                        // dbg!(&last_item);
                        last_item.push(s.to_string());
                        // }
                        // println!("Pushh {s}");
                    }
                    None => {
                        last_items.templates.insert(0, vec![s.to_string()]);
                        // println!("Pushhhhhh {s}");
                    }
                }
            }
        }
    }

    pub fn push_dynamic(&mut self, s: String) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => match &mut last.dynamics {
                Dynamics::Items(items) => {
                    if last.nested && items.is_empty() {
                        items.push(Dynamic::Nested(RenderedBuilder {
                            statics: vec![String::new(), String::new()],
                            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String(s)])),
                            templates: vec![],
                            nested: false,
                            loop_count: 0,
                        }));
                    } else {
                        items.push(Dynamic::String(s));
                        last.statics.push(String::new());
                        if last.statics.len() <= items.len() {
                            last.statics.push(String::new());
                        }
                    }
                }
                Dynamics::List(list) => match list.0.last_mut() {
                    Some(list_last) => {
                        if last.nested {
                            list_last.push(Dynamic::Nested(RenderedListItemBuilder {
                                statics: last.templates.len() - 1,
                                dynamics: vec![Dynamic::String(s)],
                                nested: true,
                                level: 0,
                            }));
                        } else {
                            list_last.push(Dynamic::String(s));
                        }
                    }
                    None => {
                        unreachable!("push_for_item should've created an item for us");
                    }
                },
            },
            LastItem::List(last) => {
                if last.nested {
                    let static_index = last.statics;
                    last.statics += 1;
                    last.dynamics.push(Dynamic::Nested(RenderedListItemBuilder {
                        statics: static_index,
                        dynamics: vec![Dynamic::String(s)],
                        nested: true,
                        level: last.level + 1,
                    }));
                } else {
                    last.dynamics.push(Dynamic::String(s));
                }
            }
        }
    }

    pub fn push_if_frame(&mut self) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => {
                last.nested = true;
                if last.statics.is_empty() {
                    last.statics.push(String::new());
                }
            }
            LastItem::List(last) => {
                // TODO
                last.nested = true;
                self.last_items_mut().templates.insert(0, vec![]);
            }
        }
    }

    pub fn push_for_frame(&mut self) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => match &mut last.dynamics {
                Dynamics::Items(items) => {
                    last.nested = true;
                    if last.statics.is_empty() {
                        last.statics.push(String::new());
                    }
                    items.push(Dynamic::Nested(RenderedBuilder {
                        statics: vec![],
                        dynamics: Dynamics::List(DynamicList(vec![])),
                        templates: vec![],
                        nested: false,
                        loop_count: 0,
                    }));
                    last.statics.push(String::new());
                }
                Dynamics::List(list) => todo!(),
            },
            LastItem::List(last) => todo!(),
        }
    }

    pub fn push_for_item(&mut self) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => match &mut last.dynamics {
                Dynamics::Items(items) => {}
                Dynamics::List(list) => {
                    list.0.push(vec![]);
                }
            },
            LastItem::List(last) => {
                // TODO
            }
        }
    }

    pub fn pop_for_item(&mut self) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => {
                last.loop_count += 1;
            }
            LastItem::List(last) => {
                // dbg!(&last);
                // todo
                // if let Some(first_template) =
                // self.last_items_mut().templates.first_mut() {
                //     first_template.push(String::new());
                // }
            }
        }
    }

    pub fn pop_frame(&mut self) {
        let last = self.last_mut();
        match last {
            LastItem::Items(last) => {
                let nested = last.nested;

                match &mut last.dynamics {
                    Dynamics::Items(items) => {
                        if last.statics.len() <= items.len() {
                            last.statics.push(String::new());
                        }

                        let parent = self.last_parent_mut();
                        if let Some(parent) = parent {
                            match parent {
                                LastItem::Items(parent) => {
                                    parent.nested = false;
                                    match &mut parent.dynamics {
                                        Dynamics::Items(items) => {
                                            parent.nested = false;
                                            if items.is_empty() {
                                                items.push(Dynamic::String(String::new()));
                                            }
                                            if parent.statics.len() <= items.len() {
                                                parent.statics.push(String::new());
                                            }
                                        }
                                        Dynamics::List(_) => todo!(),
                                    }
                                }
                                LastItem::List(_) => todo!(),
                            }
                        }
                    }
                    Dynamics::List(list) => {
                        if let Some(list_last) = list.last_mut() {
                            if nested && list_last.len() < last.statics.len() {
                                list_last.push(Dynamic::String(String::new()));
                            }

                            if last.statics.len() <= list_last.len() {
                                last.statics.push(String::new());
                            }
                        }

                        let parent = self.last_parent_mut();
                        if let Some(parent) = parent {
                            match parent {
                                LastItem::Items(parent) => {
                                    parent.nested = false;
                                    match &mut parent.dynamics {
                                        Dynamics::Items(items) => {
                                            if items.is_empty() {
                                                items.push(Dynamic::String(String::new()));
                                            }
                                            if parent.statics.len() <= items.len() {
                                                parent.statics.push(String::new());
                                            }
                                        }
                                        Dynamics::List(list) => {
                                            // dbg!(list);
                                            // dbg!(self);
                                        }
                                    }
                                }
                                LastItem::List(_) => todo!(),
                            }
                        }

                        // let parent = self.last_parent_mut();
                        // if let Some(parent) = parent {
                        //     match parent {
                        //         LastItem::Items(items) => {
                        //             items.nested = false;
                        //         }
                        //         LastItem::List(list) => {
                        //             list.nested = false;
                        //         }
                        //     }
                        // }

                        // let items = self.last_items_mut();
                        // items.
                        // if let Some(parent) = parent {
                        //     match parent {
                        //         LastItem::Items(parent) => {
                        //             parent.nested = false;
                        //             match &mut parent.dynamics {
                        //                 Dynamics::Items(items) => {
                        //                     parent.nested = false;
                        //                     if items.is_empty() {
                        //
                        // items.push(Dynamic::String(String::new()));
                        //                     }
                        //                     if parent.statics.len() <=
                        // items.len() {
                        // parent.statics.push(String::new());
                        //                     }
                        //                 }
                        //                 Dynamics::List(_) => todo!(),
                        //             }
                        //         }
                        //         LastItem::List(_) => todo!(),
                        //     }
                        // }
                    }
                }

                // Parent
            }
            LastItem::List(last) => {
                last.nested = false;

                // dbg!(&last.dynamics.len());
                // dbg!(&self.last_items_mut().templates);

                let parent = self.last_parent_mut();
                if let Some(parent) = parent {
                    match parent {
                        LastItem::Items(parent) => {
                            parent.nested = false;
                            // match &mut parent.dynamics {
                            //     Dynamics::Items(items) => {
                            //         parent.nested = false;
                            //         if items.is_empty() {
                            //
                            // items.push(Dynamic::String(String::new()));
                            //         }
                            //         if parent.statics.len() <= items.len() {
                            //             parent.statics.push(String::new());
                            //         }
                            //     }
                            //     Dynamics::List(_) => todo!(),
                            // }
                        }
                        LastItem::List(list) => {
                            // dbg!(list.dynamics.len());
                            // dbg!(&self
                            //     .last_items_mut()
                            //     .templates
                            //     .last()
                            //     .map(|first| first.len()));

                            // if items.is_empty() {
                            //     items.push(Dynamic::String(String::new()));
                            // }
                            list.nested = false;
                            let dynamics_len = list.dynamics.len();
                            let templates = &mut self.last_items_mut().templates;
                            if let Some(first_template) = templates.last_mut() {
                                if first_template.len() <= dynamics_len {
                                    first_template.push(String::new());
                                }
                            }
                            // if parent.statics.len() <= items.len() {
                            //     parent.statics.push(String::new());
                            // }
                        }
                    }
                }

                // dbg!(&self.last_items_mut().templates);
                // todo
                // match last.dynamics.last() {
                //     Some(Dynamic::Nested(list_last)) => {
                //         println!("Here");
                //     }
                //     _ => todo!(),
                // }

                // let last_items = self.last_items_mut();
                // if let Some(first_template) =
                // last_items.templates.first_mut() {
                //     if list_last.len() < first_template.len() {
                //         list_last.push(Dynamic::String(String::new()));
                //     }
                //     // if last_items.nested {
                //     first_template.push(String::new());
                //     // }
                // }
                // last_items.templates.
            }
        }
    }
}

impl RenderedListItemBuilder {
    fn next(&mut self) -> Option<&mut RenderedListItemBuilder> {
        self.dynamics.last_mut().and_then(|last| match last {
            Dynamic::String(_) => None,
            Dynamic::Nested(nested) => Some(nested),
        })
    }
}

impl From<Rendered> for RenderedBuilder {
    fn from(rendered: Rendered) -> Self {
        // RenderedBuilder {
        //     statics: rendered.statics,
        //     dynamics: rendered.dynamics.into(),
        //     nested: false,
        // }
        todo!()
    }
}

impl From<RenderedBuilder> for Rendered {
    fn from(rendered: RenderedBuilder) -> Self {
        rendered.build()
    }
}

impl From<RenderedListItemBuilder> for RenderedListItem {
    fn from(list: RenderedListItemBuilder) -> Self {
        RenderedListItem {
            statics: list.statics,
            dynamics: list
                .dynamics
                .into_iter()
                .map(|item| match item {
                    Dynamic::String(s) => Dynamic::String(s),
                    Dynamic::Nested(n) => Dynamic::Nested(n.into()),
                })
                .collect(),
        }
    }
}
