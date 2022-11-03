use serde_json::{json, Map, Value};

use super::strip::Strip;

/// provides a new `Value` containing the differences between the `old` and
/// `new`.
///
/// If the `old` Value was `Value::Null` then the `new` Value is returned BUT
/// with the `null` and `empty` values stripped as they have not been changed
/// compared to the old as there was no old and so no difference.
/// ```rust
/// use json_plus::diff;
/// use serde_json::json;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let old = json!({"key":"old value", "arr":[]});
///     let new = json!({"key":"new value", "arr":[]});
///
///     let diffed = diff(&old, &new).unwrap();
///     println!("{}", diffed.to_string());
///     Ok(())
/// }
/// ```
#[inline]
pub fn diff(old: &Value, new: &Value) -> Option<Value> {
    match old {
        Value::Null => super::strip::strip(Strip::Nulls | Strip::Empties, new.clone()),
        Value::Array(o) => {
            if new.is_array() {
                diff_array(o, new.as_array().unwrap())
            } else {
                Some(Value::Array(vec![]))
            }
        }
        Value::Object(o) if new.is_object() => diff_map(o, new),
        _ => {
            if old != new {
                if let Value::String(s) = new {
                    if s.is_empty() {
                        return Some(json!({ "d": [] }));
                    }
                }
                Some(new.clone())
            } else {
                None
            }
        }
    }
}

fn diff_array(old: &[Value], new: &[Value]) -> Option<Value> {
    if old.len() != new.len() {
        return Some(Value::Array(new.to_vec()));
    }

    for (i, o) in old.iter().enumerate() {
        if o != &new[i] {
            return Some(Value::Array(new.to_vec()));
        }
    }
    None
}

fn diff_map(old: &Map<String, Value>, new: &Value) -> Option<Value> {
    if old.is_empty() {
        return Some(new.clone());
    }

    let mut result = Map::new();
    let new_obj = new.as_object().unwrap();
    if new_obj.is_empty() {
        for (k, _) in old {
            result.insert(k.clone(), Value::Null);
        }
        return Some(Value::Object(result));
    }

    // need to go over old records first, it's the only way to know new data is no
    // longer present.
    for (k, v) in old {
        match new_obj.get(k) {
            Some(n) => match diff(v, n) {
                Some(changed) => {
                    result.insert(k.clone(), changed);
                }
                None => {
                    continue;
                }
            },
            None => {
                result.insert(k.clone(), Value::Null);
            }
        };
    }

    // check for new values that didn't exist in the old
    for (k, v) in new_obj {
        match old.get(k) {
            Some(_) => continue,
            None => {
                result.insert(k.clone(), v.clone());
            }
        }
    }

    if result.is_empty() {
        return None;
    }
    Some(Value::Object(result))
}

#[cfg(test)]
mod tests {

    use serde_json::json;

    use super::*;

    #[test]
    fn null() {
        assert_eq!(diff(&Value::Null, &Value::Null), None);
        assert_eq!(diff(&Value::Null, &true.into()), Some(Value::from(true)));
        assert_eq!(diff(&true.into(), &Value::Null), Some(Value::Null));
    }

    #[test]
    fn bool() {
        assert_eq!(diff(&true.into(), &Value::Null), Some(Value::Null));
        assert_eq!(diff(&true.into(), &true.into()), None);
        assert_eq!(diff(&Value::Null, &true.into()), Some(Value::from(true)));
        assert_eq!(diff(&false.into(), &false.into()), None);
        assert_eq!(diff(&false.into(), &true.into()), Some(Value::from(true)));
        assert_eq!(diff(&true.into(), &false.into()), Some(Value::from(false)));
    }

    #[test]
    fn string() {
        assert_eq!(diff(&"old".into(), &Value::Null), Some(Value::Null));
        assert_eq!(diff(&Value::Null, &"new".into()), Some(Value::from("new")));
        assert_eq!(diff(&"old".into(), &"old".into()), None);
        assert_eq!(diff(&"old".into(), &"new".into()), Some(Value::from("new")));
    }

    #[test]
    fn number() {
        assert_eq!(diff(&1.into(), &Value::Null), Some(Value::Null));
        assert_eq!(diff(&Value::Null, &1.into()), Some(Value::from(1)));
        assert_eq!(diff(&1.into(), &1.into()), None);
        assert_eq!(diff(&1.into(), &2.into()), Some(Value::from(2)));
    }

    #[test]
    fn array() {
        // assert_eq!(
        //     diff(&vec!["val1"].into(), &Value::Null),
        //     Some(Value::from(Vec::<&str>::new()))
        // );
        // assert_eq!(
        //     diff(&Value::Null, &vec!["val1"].into()),
        //     Some(Value::from(vec!["val1"]))
        // );
        // assert_eq!(diff(&vec!["val1"].into(), &vec!["val1"].into()), None);
        // assert_eq!(
        //     diff(&vec!["val1"].into(), &vec!["val2"].into()),
        //     Some(Value::from(vec!["val2"]))
        // );
        // assert_eq!(
        //     diff(&vec!["val1", "val2"].into(), &vec!["val1"].into()),
        //     Some(Value::from(vec!["val1"]))
        // );
        // assert_eq!(
        //     diff(&vec!["val1"].into(), &String::new().into()),
        //     Some(Value::from(Vec::<&str>::new()))
        // );
        // assert_eq!(
        //     diff(&vec!["val1"].into(), &vec!["val1", "val2"].into()),
        //     Some(Value::from(vec!["val1", "val2"]))
        // );

        let d = diff(
            &json!({ "0": { "d": [ [] ], "s": [ "<span>Hi</span>" ] }, "s": [ "", "" ] }),
            &json!({ "0": "", "s": [ "", "" ] }),
        );
        assert_eq!(d, Some(json!({ "0": { "d": [] } })));
    }

    #[test]
    fn object() {
        assert_eq!(
            diff(&json!({"key1":1,"key2":"value2"}), &Value::Null),
            Some(Value::Null)
        );
        assert_eq!(
            diff(&Value::Null, &json!({"key1":1,"key2":"value2"})),
            Some(json!({"key1":1,"key2":"value2"}))
        );
        assert_eq!(
            diff(
                &json!({"key1":1,"key2":"value2"}),
                &json!({"key1":1,"key2":"value2"})
            ),
            None
        );
        assert_eq!(
            diff(
                &json!({"key1":1,"key2":"value2","key3":[1,2],"key4":[1,2,3],"key6":true}),
                &json!({"key1":1,"key2":"value2","key3":[1,2],"key4":[1,2,3,4],"key5":true})
            ),
            Some(json!({"key4":[1,2,3,4],"key5":true,"key6":null}))
        );
        assert_eq!(
            diff(
                &json!({"M":{"a":1,"b":"foo"},"A":["foo"],"B":true}),
                &json!({"M":{"a":1,"b":"bar"},"A":["foo","bar"],"B":false})
            ),
            Some(json!({"A":["foo","bar"],"B":false,"M":{"b":"bar"}}))
        );
    }
}
