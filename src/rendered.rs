//! Rendered HTML created with the `html!` macro.

// const DYNAMICS: &str = "d";
// const STATIC: &str = "s";
// const COMPONENTS: &str = "c";
// const EVENTS: &str = "e";
// const REPLY: &str = "r";
// const TITLE: &str = "t";
// const TEMPLATES: &str = "p";

mod builder;
mod diff;
mod dynamic;
mod strip;

use core::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{map::Entry, Map, Value};

pub use self::builder::*;
use self::{
    dynamic::{Dynamic, DynamicItems, DynamicList, Dynamics},
    strip::Strip,
};

/// Rendered HTML containing statics, dynamics and templates.
///
/// Rendered is typically generated by the `html!` macro.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    statics: Vec<String>,
    dynamics: Dynamics<Self, RenderedListItem>,
    templates: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RenderedListItem {
    statics: usize,
    dynamics: Vec<Dynamics<Rendered, Self>>,
}

/// Converts a type into JSON.
pub trait IntoJson: Sized {
    /// Converts value into [`serde_json::Value`].
    fn into_json(self) -> Value {
        let mut map = Map::new();
        self.write_json(&mut map);
        map.into()
    }

    /// Writes properties to an existing map.
    fn write_json(self, _map: &mut Map<String, Value>) {
        todo!()
    }
}

impl Rendered {
    /// Creates a RenderedBuilder.
    pub fn builder() -> builder::RenderedBuilder {
        builder::RenderedBuilder::new()
    }

    /// Diffs self with another [`Rendered`] and returns diff as [`serde_json::Value`].
    pub fn diff(self, other: Rendered) -> Option<Value> {
        let a = self.into_json();
        let b = other.into_json();
        let diff = diff::diff(&a, &b).unwrap_or_default();
        match diff {
            Value::Object(_) => strip::strip(Strip::Nulls.into(), diff),
            _ => None,
        }
    }
}

impl fmt::Display for Rendered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.dynamics {
            Dynamics::Items(DynamicItems(items)) => {
                for (s, d) in self.statics.iter().zip(items.iter()) {
                    write!(f, "{s}{d}")?;
                }

                if !items.is_empty() {
                    if let Some(last) = self.statics.last() {
                        write!(f, "{last}")?;
                    }
                }
            }
            Dynamics::List(list) => {
                for dynamics in &list.0 {
                    for (s, d) in self.statics.iter().zip(dynamics.iter()) {
                        write!(f, "{s}")?;
                        fmt_dynamic_list_item(f, d, &self.templates)?;
                    }

                    if !dynamics.is_empty() {
                        if let Some(last) = self.statics.last() {
                            write!(f, "{last}")?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn fmt_dynamics(
    f: &mut fmt::Formatter<'_>,
    dynamics: &Dynamics<Rendered, RenderedListItem>,
    statics: &[String],
    templates: &[Vec<String>],
) -> fmt::Result {
    match dynamics {
        Dynamics::Items(DynamicItems(items)) => {
            for (s, d) in statics.iter().zip(items.iter()) {
                write!(f, "{s}{d}")?;
            }

            if !items.is_empty() {
                if let Some(last) = statics.last() {
                    write!(f, "{last}")?;
                }
            }
        }
        Dynamics::List(list) => {
            for dynamics in &list.0 {
                for (s, d) in statics.iter().zip(dynamics.iter()) {
                    write!(f, "{s}")?;
                    fmt_dynamic_list_item(f, d, templates)?;
                }

                if !dynamics.is_empty() {
                    if let Some(last) = statics.last() {
                        write!(f, "{last}")?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn fmt_dynamic_list_item(
    f: &mut fmt::Formatter<'_>,
    d: &Dynamic<RenderedListItem>,
    templates: &[Vec<String>],
) -> fmt::Result {
    match d {
        Dynamic::String(s) => {
            write!(f, "{s}")?;
        }
        Dynamic::Nested(n) => {
            let statics = templates.get(n.statics).unwrap();
            for (s, d) in statics.iter().zip(n.dynamics.iter()) {
                write!(f, "{s}")?;

                fmt_dynamics(f, d, &statics, templates)?;
            }

            if !n.dynamics.is_empty() {
                if let Some(last) = statics.last() {
                    write!(f, "{last}")?;
                }
            }
        }
    }

    Ok(())
}

impl IntoJson for Rendered {
    fn write_json(self, map: &mut Map<String, Value>) {
        if !self.statics.is_empty() {
            map.insert(
                "s".to_string(),
                Value::Array(self.statics.into_iter().map(|s| s.into()).collect()),
            );
        }

        if !self.templates.is_empty() {
            let mut templates_map = Map::new();
            for (i, template) in self.templates.into_iter().enumerate() {
                templates_map.insert(i.to_string(), template.into());
            }
            map.insert("p".to_string(), templates_map.into());
        }

        self.dynamics.write_json(map);
    }
}

impl IntoJson for RenderedListItem {
    fn write_json(self, map: &mut Map<String, Value>) {
        map.insert("s".to_string(), self.statics.into());

        let (items, lists): (Vec<_>, Vec<_>) = self
            .dynamics
            .into_iter()
            .map(|d| match d {
                Dynamics::Items(items) => (Some(items), None),
                Dynamics::List(list) => (None, Some(list)),
            })
            .partition(|(a, _)| a.is_some());

        let items: Vec<_> = items.into_iter().filter_map(|(i, _)| i).collect();
        let lists: Vec<_> = lists.into_iter().filter_map(|(_, l)| l).collect();

        for (i, dynamic) in items.into_iter().enumerate() {
            map.insert(i.to_string(), dynamic.into_json());
        }

        for list in lists.into_iter() {
            list.write_json(map);
        }
    }
}

impl<N, L> IntoJson for Dynamics<N, L>
where
    N: IntoJson,
    L: IntoJson,
{
    fn into_json(self) -> Value {
        match self {
            Dynamics::Items(items) => items.into_json(),
            Dynamics::List(list) => list.into_json(),
        }
    }

    fn write_json(self, map: &mut Map<String, Value>) {
        match self {
            Dynamics::Items(items) => items.write_json(map),
            Dynamics::List(list) => list.write_json(map),
        }
    }
}

impl<N> IntoJson for DynamicItems<N>
where
    N: IntoJson,
{
    fn write_json(self, map: &mut Map<String, Value>) {
        for (i, dynamic) in self.0.into_iter().enumerate() {
            map.insert(i.to_string(), dynamic.into_json());
        }
    }
}

impl<N> IntoJson for DynamicList<N>
where
    N: IntoJson,
{
    fn write_json(self, map: &mut Map<String, Value>) {
        if !self.0.iter().any(|list| !list.is_empty()) {
            return;
        }

        let dynamics = self
            .0
            .into_iter()
            .map(|list| Value::Array(list.into_iter().map(|d| d.into_json()).collect()));

        match map.entry("d".to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(dynamics.collect::<Vec<_>>().into());
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                Value::Array(array) => array.extend(dynamics),
                _ => todo!(),
            },
        }
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
