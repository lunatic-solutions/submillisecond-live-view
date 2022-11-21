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
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use self::builder::*;
use self::dynamic::{Dynamic, DynamicItems, DynamicList, Dynamics};
use self::strip::Strip;
use crate::event_handler::AnonymousEventHandler;

/// Rendered HTML containing statics, dynamics and templates.
///
/// Rendered is typically generated by the `html!` macro.
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Rendered<T> {
    statics: Vec<String>,
    dynamics: Dynamics<Self, RenderedListItem>,
    templates: Vec<Vec<String>>,
    anonymous_events: Vec<usize>,
    phantom: PhantomData<T>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RenderedListItem {
    statics: usize,
    dynamics: Vec<Dynamic<Self>>,
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

impl<T> Rendered<T> {
    /// Creates a RenderedBuilder.
    pub fn builder() -> builder::RenderedBuilder<T> {
        builder::RenderedBuilder::new()
    }

    /// Diffs self with another [`Rendered`] and returns diff as
    /// [`serde_json::Value`].
    pub fn diff<U>(self, other: Rendered<U>) -> Option<Value> {
        let a = self.into_json();
        let b = other.into_json();
        let diff = diff::diff(&a, &b).unwrap_or_default();
        match diff {
            Value::Object(_) => strip::strip(Strip::Nulls.into(), diff),
            _ => None,
        }
    }

    pub(crate) fn handle_anonymous_event(&self, state: &mut T, event_name: &str) -> bool {
        self.anonymous_events.iter().copied().any(|handler_id| {
            let handler: AnonymousEventHandler<T> = unsafe { std::mem::transmute(handler_id) };
            handler(state, event_name)
        })
    }
}

impl<T> Clone for Rendered<T> {
    fn clone(&self) -> Self {
        Self {
            statics: self.statics.clone(),
            dynamics: self.dynamics.clone(),
            templates: self.templates.clone(),
            anonymous_events: self.anonymous_events.clone(),
            phantom: PhantomData,
        }
    }
}

impl<T> fmt::Debug for Rendered<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rendered")
            .field("statics", &self.statics)
            .field("dynamics", &self.dynamics)
            .field("templates", &self.templates)
            .field("anonymous_events", &self.anonymous_events.len())
            .finish()
    }
}

impl<T> fmt::Display for Rendered<T> {
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
                        fmt_dynamic_list_item(&self.templates, d, f)?;
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

impl<T> PartialEq for Rendered<T> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.dynamics == other.dynamics
            && self.templates == other.templates
            && self.anonymous_events == other.anonymous_events
    }
}

fn fmt_dynamic_list_item(
    templates: &Vec<Vec<String>>,
    d: &Dynamic<RenderedListItem>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match d {
        Dynamic::String(s) => {
            write!(f, "{s}")?;
        }
        Dynamic::Nested(n) => {
            let statics = templates.get(n.statics).unwrap();
            for (s, d) in statics.iter().zip(n.dynamics.iter()) {
                write!(f, "{s}")?;
                fmt_dynamic_list_item(templates, d, f)?;
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

impl<T> IntoJson for Rendered<T> {
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

        for (i, dynamic) in self.dynamics.into_iter().enumerate() {
            map.insert(i.to_string(), dynamic.into_json());
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
    fn into_json(self) -> Value {
        todo!()
    }

    fn write_json(self, map: &mut Map<String, Value>) {
        let dynamics = Value::Array(
            self.0
                .into_iter()
                .map(|dynamic| {
                    Value::Array(
                        dynamic
                            .into_iter()
                            .map(|dynamic| dynamic.into_json())
                            .collect(),
                    )
                })
                .collect(),
        );

        map.insert("d".to_string(), dynamics);

        // for dynamics in self.0 {
        //     map.insert(k, v)
        // }
        // todo!()
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
