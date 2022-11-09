use std::{fmt, ops};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Dynamics<N, L> {
    Items(DynamicItems<N>),
    List(DynamicList<L>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DynamicItems<N>(pub Vec<Dynamic<N>>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DynamicList<L>(pub Vec<Vec<Dynamic<L>>>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Dynamic<N> {
    String(String),
    Nested(N),
}

impl<N> ops::Deref for DynamicItems<N> {
    type Target = Vec<Dynamic<N>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N> ops::DerefMut for DynamicItems<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<L> ops::Deref for DynamicList<L> {
    type Target = Vec<Vec<Dynamic<L>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<L> ops::DerefMut for DynamicList<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<N> fmt::Display for Dynamic<N>
where
    N: fmt::Display,
{
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
