use std::{fmt, ops};

use serde::{Deserialize, Serialize};

use super::{Rendered, RenderedBuilder, RenderedDiff, RenderedListItem, RenderedListItemBuilder};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dynamics<N, L> {
    Items(DynamicItems<N>),
    List(DynamicList<L>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicItems<N>(pub Vec<Dynamic<N>>);

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicList<L>(pub Vec<Vec<Dynamic<L>>>);

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dynamic<N> {
    String(String),
    Nested(N),
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

macro_rules! impl_from_dynamics {
    ($a: ty, $b: ty) => {
        impl From<Dynamics<$a>> for Dynamics<$b> {
            fn from(dynamics: Dynamics<$a>) -> Self {
                match dynamics {
                    Dynamics::Individual(dynamics) => {
                        Dynamics::Individual(dynamics.into_iter().map(Dynamic::from).collect())
                    }
                    Dynamics::List(list) => Dynamics::List(
                        list.into_iter()
                            .map(|dynamics| dynamics.into_iter().map(Dynamic::from).collect())
                            .collect(),
                    ),
                }
            }
        }

        impl From<Dynamic<$a>> for Dynamic<$b> {
            fn from(dynamic: Dynamic<$a>) -> Self {
                match dynamic {
                    Dynamic::String(s) => Dynamic::String(s),
                    Dynamic::Nested(n) => Dynamic::Nested(n.into()),
                }
            }
        }
    };
}

// impl_from_dynamics!(Rendered, RenderedBuilder);
// impl_from_dynamics!(Rendered, RenderedDiff);
// impl_from_dynamics!(Rendered, RenderedDiff);

macro_rules! impl_from_dynamic {
    ($a: ty, $b: ty) => {
        impl From<Dynamic<$a>> for Dynamic<$b> {
            fn from(dynamic: Dynamic<$a>) -> Self {
                match dynamic {
                    Dynamic::String(s) => Dynamic::String(s),
                    Dynamic::Nested(n) => Dynamic::Nested(n.into()),
                }
            }
        }
    };
}

impl_from_dynamic!(RenderedBuilder, Rendered);
impl_from_dynamic!(RenderedListItemBuilder, RenderedListItem);
impl_from_dynamic!(Rendered, RenderedDiff);
