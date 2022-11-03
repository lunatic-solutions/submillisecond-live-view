// const DYNAMICS: &str = "d";
// const STATIC: &str = "s";
// const COMPONENTS: &str = "c";
// const EVENTS: &str = "e";
// const REPLY: &str = "r";
// const TITLE: &str = "t";
// const TEMPLATES: &str = "p";

mod builder;
mod dynamic;

use core::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use self::builder::*;
pub use self::dynamic::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    pub statics: Vec<String>,
    pub dynamics: Dynamics<Self, RenderedListItem>,
    pub templates: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedListItem {
    pub statics: usize,
    pub dynamics: Vec<Dynamic<Self>>,
}

impl Rendered {
    pub fn builder() -> builder::RenderedBuilder {
        builder::RenderedBuilder::new()
    }

    pub fn diff(self, other: Rendered) -> Value {
        let a = self.into_json();
        let b = other.into_json();
        json_plus::diff(&a, &b).unwrap_or_default()
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

pub trait IntoJson {
    fn into_json(self) -> Value;
}

impl IntoJson for Rendered {
    fn into_json(self) -> Value {
        let mut map = Map::new();
        if !self.statics.is_empty() {
            map.insert(
                "s".to_string(),
                Value::Array(self.statics.into_iter().map(|s| s.into()).collect()),
            );
        }
        let mut json: Value = map.into();
        let dynamics = self.dynamics.into_json();
        json_plus::merge(&mut json, &dynamics);

        json
    }
}

impl IntoJson for RenderedListItem {
    fn into_json(self) -> Value {
        todo!()
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
}

impl<N> IntoJson for DynamicItems<N>
where
    N: IntoJson,
{
    fn into_json(self) -> Value {
        let mut map = Map::new();
        for (i, dynamic) in self.0.into_iter().enumerate() {
            map.insert(i.to_string(), dynamic.into_json());
        }
        map.into()
    }
}

impl<N> IntoJson for DynamicList<N>
where
    N: IntoJson,
{
    fn into_json(self) -> Value {
        todo!()
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
