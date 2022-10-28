// const DYNAMICS: &str = "d";
// const STATIC: &str = "s";
// const COMPONENTS: &str = "c";
// const EVENTS: &str = "e";
// const REPLY: &str = "r";
// const TITLE: &str = "t";
// const TEMPLATES: &str = "p";

use core::fmt;
use std::collections::HashMap;

use itertools::{EitherOrBoth, Itertools};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    pub statics: Vec<String>,
    pub dynamics: Vec<Dynamic>,
}

impl Rendered {
    pub fn builder() -> RenderedBuilder {
        RenderedBuilder::default()
    }
}

pub trait IntoJson {
    fn into_json(self) -> Value;
}

impl IntoJson for Rendered {
    fn into_json(self) -> Value {
        let this: RenderedDiff = self.into();
        this.into_json()
    }
}

impl IntoJson for RenderedDiff {
    fn into_json(self) -> Value {
        let mut map = Map::new();
        if let Some(s) = self.statics {
            map.insert(
                "s".to_string(),
                Value::Array(s.into_iter().map(|s| s.into()).collect()),
            );
        }
        for (i, d) in self.dynamics {
            map.insert(i.to_string(), d.into_json());
        }
        map.into()
    }
}

impl<N> IntoJson for Dynamic<N>
where
    N: IntoJson,
{
    fn into_json(self) -> Value {
        match self {
            Dynamic::String(s) => s.into(),
            Dynamic::Nested(n) => n.into_json(),
        }
    }
}

pub trait DiffRender<Rhs> {
    fn diff(self, other: Rhs) -> RenderedDiff;
}

impl<Rhs> DiffRender<Rhs> for Rendered
where
    Rhs: Into<RenderedDiff>,
{
    fn diff(self, other: Rhs) -> RenderedDiff {
        let this: RenderedDiff = self.into();
        this.diff(other)
    }
}

impl<Rhs> DiffRender<Rhs> for RenderedDiff
where
    Rhs: Into<RenderedDiff>,
{
    fn diff(self, other: Rhs) -> RenderedDiff {
        let other: RenderedDiff = other.into();

        if self.statics != other.statics {
            return other;
        }

        let dynamics = self
            .dynamics
            .into_iter()
            .sorted_by(|(a, _), (b, _)| Ord::cmp(a, b))
            .zip_longest(
                other
                    .dynamics
                    .into_iter()
                    .sorted_by(|(a, _), (b, _)| Ord::cmp(a, b)),
            )
            .filter_map(|zip| match zip {
                EitherOrBoth::Both((i, dynamic_a), (_, dynamic_b)) => {
                    match (dynamic_a, dynamic_b) {
                        (Dynamic::String(a), Dynamic::String(b)) => {
                            if a != b {
                                Some((i, Dynamic::String(b)))
                            } else {
                                None
                            }
                        }
                        (Dynamic::String(_), b @ Dynamic::Nested(_))
                        | (Dynamic::Nested(_), b @ Dynamic::String(_)) => Some((i, b)),
                        (Dynamic::Nested(a), Dynamic::Nested(b)) => {
                            let diff = a.diff(b);
                            if diff.statics.is_none() && diff.dynamics.is_empty() {
                                None
                            } else {
                                Some((i, Dynamic::Nested(diff)))
                            }
                        }
                    }
                }
                EitherOrBoth::Left((_i, _dynamic)) => {
                    // Item was deleted but this shouldn't happen.
                    None
                }
                EitherOrBoth::Right((i, dynamic)) => Some((i, dynamic)),
            })
            .collect();

        RenderedDiff {
            statics: None,
            dynamics,
        }
    }
}

impl fmt::Display for Rendered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (s, d) in self.statics.iter().zip(self.dynamics.iter()) {
            write!(f, "{s}{d}")?;
        }

        if !self.dynamics.is_empty() {
            if let Some(last) = self.statics.last() {
                write!(f, "{last}")?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dynamic<N = Rendered> {
    String(String),
    Nested(N),
}

impl fmt::Display for Dynamic<Rendered> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dynamic::String(s) => {
                write!(f, "{s}")
            }
            Dynamic::Nested(n) => {
                write!(f, "{n}")
            }
        }
    }
}

pub struct RenderedDiff {
    pub statics: Option<Vec<String>>,
    pub dynamics: HashMap<usize, Dynamic<Self>>,
}

impl From<Rendered> for RenderedDiff {
    fn from(rendered: Rendered) -> Self {
        RenderedDiff {
            statics: Some(rendered.statics),
            dynamics: rendered
                .dynamics
                .into_iter()
                .enumerate()
                .map(|(i, dynamic)| (i, Dynamic::<RenderedDiff>::from(dynamic)))
                .collect(),
        }
    }
}

impl From<Dynamic<Rendered>> for Dynamic<RenderedDiff> {
    fn from(d: Dynamic<Rendered>) -> Self {
        match d {
            Dynamic::String(s) => Dynamic::String(s),
            Dynamic::Nested(n) => Dynamic::Nested(n.into()),
        }
    }
}

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
