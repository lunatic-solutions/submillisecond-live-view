use std::fmt;

use serde::{Deserialize, Serialize};

use super::Rendered;

// #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// pub enum Dynamics<N = Rendered> {
//     Individual(Vec<Dynamic<N>>),
//     List(Vec<Vec<Dynamic<N>>>),
// }

// impl Default for Dynamics {
//     fn default() -> Self {
//         Dynamics::Individual(vec![])
//     }
// }

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
