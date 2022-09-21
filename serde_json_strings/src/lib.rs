use std::{convert::TryFrom, marker::PhantomData, str::FromStr};

use serde::{de::Visitor, Deserialize, Deserializer};
use serde_json::Value;

struct ValueStrings(Value);

impl<'de> Deserialize<'de> for ValueStrings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde_json::Deserializer::from_str("").deserialize_map(visitor)

        #[derive(Default)]
        struct ValueStringsVisitor<T> {
            phantom: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for ValueStringsVisitor<T>
        where
            T: FromStr,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                todo!()
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.parse().map_err(|_| {
                    <E as serde::de::Error>::custom(format!(
                        "cannot convert string into {}",
                        std::any::type_name::<T>()
                    ))
                })
            }
        }

        deserializer.deserialize_any(ValueStringsVisitor::default())
    }
}
