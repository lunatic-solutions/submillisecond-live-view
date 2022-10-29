use super::{Dynamic, Rendered};

#[derive(Debug, Default)]
pub struct RenderedBuilder {
    statics: Vec<String>,
    dynamics: Vec<Dynamic<Self>>,
    nested: bool,
}

impl RenderedBuilder {
    pub fn build(self) -> Rendered {
        Rendered {
            statics: self.statics,
            dynamics: self
                .dynamics
                .into_iter()
                .map(|d| match d {
                    Dynamic::String(s) => Dynamic::String(s),
                    Dynamic::Nested(n) => Dynamic::Nested(n.build()),
                })
                .collect(),
        }
    }

    fn last_mut(&mut self) -> &mut Self {
        let mut current = self as *mut Self;

        loop {
            // SAFETY: Rust doesn't like this, though it is safe in this case.
            // This works in polonius, but not Rust's default borrow checker.
            unsafe {
                if !(*current).nested {
                    return &mut *current;
                }

                let next = (*current).dynamics.last_mut().and_then(|last| match last {
                    Dynamic::String(_) => None,
                    Dynamic::Nested(nested) => Some(nested),
                });
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

    fn last_parent_mut(&mut self) -> Option<&mut Self> {
        if !self.nested {
            return None;
        }

        let mut current = self;
        loop {
            let next = current.dynamics.last_mut().and_then(|last| match last {
                Dynamic::String(_) => None,
                Dynamic::Nested(nested) => Some(nested),
            });
            if !next.map(|next| next.nested).unwrap_or(false) {
                return Some(current);
            }
            match current.dynamics.last_mut() {
                Some(Dynamic::Nested(nested)) => current = nested,
                _ => unreachable!(),
            }
        }
    }

    pub fn push_nested(&mut self, nested: Rendered) {
        let last = self.last_mut();
        let nested: RenderedBuilder = nested.into();
        let mut statics = nested.statics.into_iter();
        if let Some(first_static) = statics.next() {
            match last.statics.last_mut() {
                Some(static_string) => static_string.push_str(&first_static),
                None => last.statics.push(first_static),
            }
            last.statics.extend(statics);
        }
        last.dynamics.extend(nested.dynamics);
    }

    pub fn push_static(&mut self, s: &str) {
        let last = self.last_mut();
        if last.nested && last.dynamics.is_empty() {
            last.dynamics.push(Dynamic::Nested(RenderedBuilder {
                statics: vec![s.to_string()],
                dynamics: vec![],
                nested: false,
            }));
        } else if last.statics.len() >= last.dynamics.len() {
            match last.statics.last_mut() {
                Some(static_string) => static_string.push_str(s),
                None => last.statics.push(s.to_string()),
            }
        } else {
            last.statics.push(s.to_string());
        }
    }

    pub fn push_dynamic(&mut self, s: String) {
        let last = self.last_mut();
        if last.nested && last.dynamics.is_empty() {
            last.dynamics.push(Dynamic::Nested(RenderedBuilder {
                statics: vec![String::new(), String::new()],
                dynamics: vec![Dynamic::String(s)],
                nested: false,
            }));
        } else {
            last.dynamics.push(Dynamic::String(s));
            last.statics.push(String::new());
            if last.statics.len() <= last.dynamics.len() {
                last.statics.push(String::new());
            }
        }
    }

    pub fn push_if_frame(&mut self) {
        let mut last = self.last_mut();
        last.nested = true;
        if last.statics.is_empty() {
            last.statics.push(String::new());
        }
    }

    pub fn push_for_frame(&mut self) {
        let mut last = self.last_mut();
        last.nested = true;
        if last.statics.is_empty() {
            last.statics.push(String::new());
        }
        last.dynamics
            .push(Dynamic::Nested(RenderedBuilder::default()));
        last.statics.push(String::new());
    }

    pub fn pop_frame(&mut self) {
        let last = self.last_mut();
        if last.statics.len() <= last.dynamics.len() {
            last.statics.push(String::new());
        }

        // Parent
        let parent = self.last_parent_mut();
        if let Some(parent) = parent {
            parent.nested = false;
            if parent.dynamics.is_empty() {
                parent.dynamics.push(Dynamic::String(String::new()));
            }
            if parent.statics.len() <= parent.dynamics.len() {
                parent.statics.push(String::new());
            }
        }
    }
}

impl From<Rendered> for RenderedBuilder {
    fn from(rendered: Rendered) -> Self {
        RenderedBuilder {
            statics: rendered.statics,
            dynamics: rendered
                .dynamics
                .into_iter()
                .map(|d| match d {
                    Dynamic::String(s) => Dynamic::String(s),
                    Dynamic::Nested(n) => Dynamic::Nested(n.into()),
                })
                .collect(),
            nested: false,
        }
    }
}
