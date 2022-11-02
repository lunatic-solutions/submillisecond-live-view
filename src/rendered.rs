// const DYNAMICS: &str = "d";
// const STATIC: &str = "s";
// const COMPONENTS: &str = "c";
// const EVENTS: &str = "e";
// const REPLY: &str = "r";
// const TITLE: &str = "t";
// const TEMPLATES: &str = "p";

mod builder;
mod builder_new;
mod diff;
mod dynamic;

use core::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use self::builder::*;
pub use self::diff::*;
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
    pub fn builder() -> builder_new::Tree {
        builder_new::Tree::new()
    }
}

impl fmt::Display for Rendered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // for (s, d) in self.statics.iter().zip(self.dynamics.iter()) {
        //     write!(f, "{s}{d}")?;
        // }

        // if !self.dynamics.is_empty() {
        //     if let Some(last) = self.statics.last() {
        //         write!(f, "{last}")?;
        //     }
        // }

        // Ok(())
        todo!()
    }
}

pub trait IntoJson {
    fn into_json(self) -> Value;
}

// impl IntoJson for Rendered {
//     fn into_json(self) -> Value {
//         let this: RenderedDiff = self.into();
//         this.into_json()
//     }
// }

// impl IntoJson for RenderedDiff {
//     fn into_json(self) -> Value {
//         let mut map = Map::new();
//         if !self.statics.is_empty() {
//             map.insert(
//                 "s".to_string(),
//                 Value::Array(self.statics.into_iter().map(|s|
// s.into()).collect()),             );
//         }
//         for (i, d) in self.dynamics {
//             map.insert(i.to_string(), d.into_json());
//         }
//         map.into()
//     }
// }

// impl<N> IntoJson for Dynamic<N>
// where
//     N: IntoJson,
// {
//     fn into_json(self) -> Value {
//         match self {
//             Dynamic::String(s) => s.into(),
//             Dynamic::Nested(n) => n.into_json(),
//         }
//     }
// }
