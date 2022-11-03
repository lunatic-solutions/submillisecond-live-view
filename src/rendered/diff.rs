use std::collections::HashMap;

use itertools::{EitherOrBoth, Itertools};

use super::{Dynamic, DynamicItems, Dynamics, Rendered};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedDiff {
    dynamics: DynamicsDiff,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedListItemDiff {
    statics: usize,
    dynamics: Vec<Dynamic<Self>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DynamicsDiff {
    Items(DynamicItemsDiff),
    List(DynamicListDiff),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DynamicItemsDiff(Vec<Option<Dynamic<RenderedDiff>>>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct DynamicListDiff(Vec<Vec<Option<Dynamic<RenderedListItemDiff>>>>);

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
                            if diff.statics.is_empty() && diff.dynamics.is_empty() {
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
            statics: vec![],
            dynamics,
        }
    }
}

impl From<Rendered> for RenderedDiff {
    fn from(rendered: Rendered) -> Self {
        let dynamics = match rendered.dynamics {
            Dynamics::Items(items) => items
                .0
                .into_iter()
                .enumerate()
                .map(|(i, dynamic)| (i, Dynamic::from(dynamic)))
                .collect(),
            Dynamics::List(list) => todo!(),
        };

        RenderedDiff {
            statics: rendered.statics,
            dynamics,
        }
    }
}
