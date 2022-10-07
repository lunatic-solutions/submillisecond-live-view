use std::collections::HashMap;

use itertools::{EitherOrBoth, Itertools};
use serde::de::Visitor;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

use super::{DynamicRender, Rendered};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderedJson {
    pub statics: Option<Vec<String>>,
    pub dynamics: HashMap<usize, DynamicRenderJson>,
}

impl RenderedJson {
    pub fn diff(&self, other: &RenderedJson) -> RenderedJson {
        if self.statics != other.statics {
            return other.clone();
        }

        let dynamics = self
            .dynamics
            .iter()
            .zip_longest(other.dynamics.iter())
            .filter_map(|zip| match zip {
                EitherOrBoth::Both((i, dynamic_a), (_, dynamic_b)) => {
                    match (dynamic_a, dynamic_b) {
                        (DynamicRenderJson::String(a), DynamicRenderJson::String(b)) => {
                            if a != b {
                                Some((*i, DynamicRenderJson::String(b.clone())))
                            } else {
                                None
                            }
                        }
                        (DynamicRenderJson::String(_), b @ DynamicRenderJson::Nested(_))
                        | (DynamicRenderJson::Nested(_), b @ DynamicRenderJson::String(_)) => {
                            Some((*i, b.clone()))
                        }
                        (DynamicRenderJson::Nested(a), DynamicRenderJson::Nested(b)) => {
                            let diff = a.diff(b);
                            if diff.statics.is_none() && diff.dynamics.is_empty() {
                                None
                            } else {
                                Some((*i, DynamicRenderJson::Nested(diff)))
                            }
                        }
                    }
                }
                EitherOrBoth::Left((_i, _dynamic)) => {
                    // Item was deleted but this shouldn't happen.
                    None
                }
                EitherOrBoth::Right((i, dynamic)) => Some((*i, dynamic.clone())),
            })
            .collect();

        RenderedJson {
            statics: None,
            dynamics,
        }
    }
}

// #[derive(Default)]
// pub struct DynamicDiff {
//     map: HashMap<usize, DynamicDiffValue>,
// }

// // impl From<RenderedJson> for DynamicDiff {
// //     fn from(rendered: RenderedJson) -> Self {
// //         rendered.dynamics
// //     }
// // }

// enum DynamicDiffValue {
//     String(String),
//     Rendered(DynamicRenderedJson),
//     Nested(Box<DynamicDiff>),
// }

// impl From<RenderedJson> for Rendered {
//     fn from(rendered_json: RenderedJson) -> Self {
//         Rendered {
//             statics: rendered_json.statics.un,
//             dynamics: rendered_json
//                 .dynamics
//                 .into_iter()
//                 .map(|(_i, dynamic)| DynamicRender::from(dynamic))
//                 .collect(),
//             nested: false,
//         }
//     }
// }

impl From<Rendered> for RenderedJson {
    fn from(rendered: Rendered) -> Self {
        RenderedJson {
            statics: Some(rendered.statics),
            dynamics: rendered
                .dynamics
                .into_iter()
                .enumerate()
                .map(|(i, dynamic)| (i, DynamicRenderJson::from(dynamic)))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DynamicRenderJson {
    String(String),
    Nested(RenderedJson),
}

// impl From<DynamicRenderJson> for DynamicRender {
//     fn from(dynamic_render_json: DynamicRenderJson) -> Self {
//         match dynamic_render_json {
//             DynamicRenderJson::String(s) => DynamicRender::String(s),
//             DynamicRenderJson::Nested(rendered_json) =>
// DynamicRender::Nested(rendered_json.into()),         }
//     }
// }

impl From<DynamicRender> for DynamicRenderJson {
    fn from(dynamic_render: DynamicRender) -> Self {
        match dynamic_render {
            DynamicRender::String(s) => DynamicRenderJson::String(s),
            DynamicRender::Nested(rendered) => DynamicRenderJson::Nested(rendered.into()),
        }
    }
}

impl Serialize for RenderedJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(
            self.dynamics.len() + self.statics.as_ref().map(|_| 1).unwrap_or(0),
        ))?;
        if let Some(statics) = &self.statics {
            map.serialize_entry("s", statics)?;
        }
        for (i, value) in self.dynamics.iter() {
            map.serialize_entry(&i, value)?;
        }
        map.end()
    }
}

impl Serialize for DynamicRenderJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            DynamicRenderJson::String(s) => serializer.serialize_str(s),
            DynamicRenderJson::Nested(rendered) => rendered.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RenderedJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RenderedWrapper {
            rendered: RenderedMap,
        }

        let rendered_outer = RenderedWrapper::deserialize(deserializer)?;
        Ok(rendered_outer.rendered.rendered)
    }
}

struct RenderedMap {
    rendered: RenderedJson,
}

impl<'de> Deserialize<'de> for RenderedMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(RenderedMapVisitor)
    }
}

struct RenderedMapVisitor;

impl<'de> Visitor<'de> for RenderedMapVisitor {
    type Value = RenderedMap;

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let (_, statics): (String, Vec<String>) = map
            .next_entry()?
            .ok_or_else(|| <A::Error as serde::de::Error>::missing_field("s"))?;
        let dynamics =
            std::iter::from_fn(|| map.next_entry::<usize, DynamicRenderJson>().transpose())
                .collect::<Result<_, _>>()?;

        Ok(RenderedMap {
            rendered: RenderedJson {
                dynamics,
                statics: Some(statics),
            },
        })
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a map of dynamic values and a static array")
    }
}

impl<'de> Deserialize<'de> for DynamicRenderJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DynamicRenderVisitor;

        impl<'de> Visitor<'de> for DynamicRenderVisitor {
            type Value = DynamicRenderJson;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a string or map")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(DynamicRenderJson::String(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(DynamicRenderJson::String(v))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                Ok(DynamicRenderJson::Nested(
                    RenderedMapVisitor.visit_map(map)?.rendered,
                ))
            }
        }

        deserializer.deserialize_any(DynamicRenderVisitor)
    }
}
