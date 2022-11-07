use enumflags2::{bitflags, BitFlags};
use serde_json::{Map, Value};

/// Specifies the type of strip operation to perform using Bitwise OR eg.
/// Strip::Nulls | Strip::Empties
#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Strip {
    Nulls,
    Empties,
}

/// Strips the provided value of specified Strip enum type.
///
/// Note: This does NOT remove nulls inside arrays unless ALL are null and the
/// [Strip] `Null` | `Empties` options are set due to the potential for
/// re-ordering indexes where each may have a specific meaning.
/// ```rust
/// use json_plus::{strip, Strip};
/// use serde_json::json;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let base = json!({"key":"old value", "null":null, "empty":[]});
///
///     let stripped = strip(Strip::Nulls | Strip::Empties, base).unwrap();
///     println!("{}", stripped.to_string());
///     Ok(())
/// }
/// ```
#[inline]
pub fn strip(mask: BitFlags<Strip, u8>, mut value: Value) -> Option<Value> {
    match strip_mut_inner(mask, &mut value) {
        false => None,
        true => Some(value),
    }
}

fn strip_mut_inner(mask: BitFlags<Strip, u8>, value: &mut Value) -> bool {
    match value {
        Value::Null => !mask.intersects(Strip::Nulls),
        Value::Object(ref mut o) => {
            o.retain(|_, v| strip_mut_inner(mask, v));
            !(o.is_empty() && mask.intersects(Strip::Empties))
        }
        Value::Array(a) => {
            // We do NOT remove nulls inside arrays unless ALL are null and null | empties
            // is set due to the potential for re-ordering indexes where each
            // may have a specific meaning.
            let mut null_count = 0;
            for value in a.iter_mut() {
                if !strip_mut_inner(mask, value) {
                    *value = Value::Null;
                    null_count += 1;
                }
            }
            if null_count == a.len() {
                a.clear();
            }

            !(a.is_empty() && mask.intersects(Strip::Empties))
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn strip_it() {
        assert_eq!(strip(Strip::Nulls.into(), json!(null)), None);
        assert_eq!(strip(Strip::Nulls | Strip::Empties, json!(null)), None);
        assert_eq!(strip(Strip::Nulls.into(), json!({})), Some(json!({})));
        assert_eq!(strip(Strip::Nulls | Strip::Empties, json!({})), None);
        assert_eq!(
            strip(
                Strip::Nulls | Strip::Empties,
                json!({"key":{"value":"value", "null":null}, "arr":[null]})
            ),
            Some(json!({"key":{"value":"value"}}))
        );
        assert_eq!(
            strip(
                Strip::Nulls | Strip::Empties,
                json!({"key":{"value":"value", "null":null}, "arr":[null, 1]})
            ),
            Some(json!({"key":{"value":"value"}, "arr":[null, 1]}))
        );
        assert_eq!(
            strip(
                Strip::Nulls | Strip::Empties,
                json!({"key":{"value":"value", "null":null}, "arr":[]})
            ),
            Some(json!({"key":{"value":"value"}}))
        );
        assert_eq!(
            strip(
                Strip::Empties.into(),
                json!({"key":{"value":null, "null":null}, "arr":[null], "empty":[]})
            ),
            Some(json!({"key":{"value":null, "null":null}, "arr":[null]}))
        );
    }
}
