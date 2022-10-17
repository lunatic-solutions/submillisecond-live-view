use std::borrow::Cow;
use std::io;

use serde::{Deserialize, Serialize};
use tera::RenderVisitor;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    pub statics: Vec<String>,
    pub dynamics: Vec<DynamicRender>,
    pub nested: bool,
}

impl Rendered {
    fn last_mut(&mut self) -> &mut Rendered {
        let mut current = self as *mut Self;

        loop {
            // SAFETY: Rust doesn't like this, though it is safe in this case.
            // This works in polonius, but not Rust's default borrow checker.
            unsafe {
                if !(*current).nested {
                    return &mut *current;
                }

                let next = (*current).dynamics.last_mut().and_then(|last| match last {
                    DynamicRender::String(_) => None,
                    DynamicRender::Nested(nested) => Some(nested),
                });
                match next {
                    Some(next) => {
                        current = next;
                    }
                    None => {
                        return &mut *current;
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DynamicRender {
    String(String),
    Nested(Rendered),
}

impl RenderVisitor for Rendered {
    fn write_static(&mut self, s: Cow<'_, str>) -> io::Result<()> {
        let last = self.last_mut();
        if last.statics.len() >= last.dynamics.len() {
            match last.statics.last_mut() {
                Some(static_string) => static_string.push_str(&s),
                None => last.statics.push(s.into_owned()),
            }
        } else {
            last.statics.push(s.into_owned());
        }

        Ok(())
    }

    fn write_dynamic(&mut self, s: Cow<'_, str>) -> io::Result<()> {
        let last = self.last_mut();
        if last.statics.is_empty() {
            last.statics.push("".to_string());
        }

        last.dynamics.push(DynamicRender::String(s.into_owned()));

        if last.statics.len() <= last.dynamics.len() {
            last.statics.push("".to_string());
        }

        Ok(())
    }

    fn push_for_loop_frame(&mut self) {
        let mut last = self.last_mut();
        last.nested = true;
        if last.statics.is_empty() {
            last.statics.push("".to_string());
        }
        last.dynamics
            .push(DynamicRender::Nested(Rendered::default()));
        last.statics.push("".to_string());
    }

    fn push_if_frame(&mut self) {
        let mut last = self.last_mut();
        last.nested = true;
        if last.statics.is_empty() {
            last.statics.push("".to_string());
        }
        last.dynamics
            .push(DynamicRender::Nested(Rendered::default()));
        last.statics.push("".to_string());
    }

    fn pop(&mut self) {
        let mut last = self.last_mut();
        last.nested = false;
        if last.statics.len() <= last.dynamics.len() {
            last.statics.push("".to_string());
        }

        // Parent
        last = self.last_mut();
        last.nested = false;
        if last.statics.len() <= last.dynamics.len() {
            last.statics.push("".to_string());
        }
    }
}

#[cfg(test)]
mod template_tests {
    use serde_json::{json, Value};
    use tera::{Context, Tera};

    use super::Rendered;
    use crate::tera::rendered_json::{DynamicRenderJson, RenderedJson};

    fn render_template(content: &str, context: Value) -> RenderedJson {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![]);
        tera.add_raw_template("test", content).unwrap();
        let mut render = Rendered::default();
        tera.render_to("test", &Context::from_value(context).unwrap(), &mut render)
            .unwrap();

        render.into()
    }

    macro_rules! assert_eq_dynamics {
        ($dynamics: expr, $vec: expr) => {
            assert_eq!($dynamics, $vec.into_iter().enumerate().collect())
        };
    }

    #[lunatic::test]
    fn template_basic() {
        let render = render_template("Hello", json!({}));

        assert_eq!(render.statics, Some(vec!["Hello".to_string()]));
        assert!(render.dynamics.is_empty());
    }

    #[lunatic::test]
    fn template_with_variable() {
        let render = render_template(
            "Hello {{ name }}",
            json!({
                "name": "Bob",
            }),
        );

        assert_eq!(
            render.statics,
            Some(vec!["Hello ".to_string(), "".to_string()])
        );
        assert_eq_dynamics!(
            render.dynamics,
            [DynamicRenderJson::String("Bob".to_string())]
        );
    }

    #[lunatic::test]
    fn template_with_multiple_variables() {
        let render = render_template(
            "Hello {{ name }}, you are {{ age }} years old",
            json!({
                "name": "Bob",
                "age": 22,
            }),
        );

        assert_eq!(
            render.statics,
            Some(vec![
                "Hello ".to_string(),
                ", you are ".to_string(),
                " years old".to_string()
            ])
        );
        assert_eq_dynamics!(
            render.dynamics,
            [
                DynamicRenderJson::String("Bob".to_string()),
                DynamicRenderJson::String("22".to_string())
            ]
        );
    }

    #[lunatic::test]
    fn template_with_if_statement() {
        let render = render_template(
            "Welcome {% if user %}{{ user }}{% else %}stranger{% endif %}",
            json!({
                "user": "Bob",
            }),
        );

        assert_eq!(
            render.statics,
            Some(vec!["Welcome ".to_string(), "".to_string()])
        );
        assert_eq_dynamics!(
            render.dynamics,
            [DynamicRenderJson::Nested(RenderedJson {
                statics: Some(vec!["".to_string(), "".to_string()]),
                dynamics: [DynamicRenderJson::String("Bob".to_string())]
                    .into_iter()
                    .enumerate()
                    .collect()
            })]
        );
    }

    #[lunatic::test]
    fn template_with_nested_if_statement() {
        let render = render_template(
            r#"
                {%- if count >= 1 -%}
                    <p>Count is high!</p>
                    {%- if count >= 2 -%}
                        <p>Count is very high!</p>
                    {%- endif -%}
                {%- endif -%}
            "#,
            json!({
                "count": 0,
            }),
        );

        assert_eq!(render.statics, Some(vec!["".to_string(), "".to_string()]));
        assert_eq_dynamics!(render.dynamics, [DynamicRenderJson::String("".to_string())]);
    }
}

#[cfg(test)]
mod template_diff_tests {
    use std::collections::HashMap;

    use serde_json::{json, Value};
    use tera::{Context, Tera};

    use super::Rendered;
    use crate::tera::rendered_json::{DynamicRenderJson, RenderedJson};

    fn render_template_diff(content: &str, context_a: Value, context_b: Value) -> RenderedJson {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![]);
        tera.add_raw_template("test", content).unwrap();
        let mut render_a = Rendered::default();
        tera.render_to(
            "test",
            &Context::from_value(context_a).unwrap(),
            &mut render_a,
        )
        .unwrap();

        let mut render_b = Rendered::default();
        tera.render_to(
            "test",
            &Context::from_value(context_b).unwrap(),
            &mut render_b,
        )
        .unwrap();

        let a = RenderedJson::from(render_a);
        let b = RenderedJson::from(render_b);

        a.diff(&b)
    }

    macro_rules! assert_eq_dynamics {
        ($dynamics: expr, $vec: expr) => {
            assert_eq!($dynamics, $vec.into_iter().collect())
        };
    }

    #[lunatic::test]
    fn template_diff_with_variable() {
        let diff = render_template_diff(
            "Hello {{ name }}",
            json!({
                "name": "Bob",
            }),
            json!({
                "name": "Jim",
            }),
        );

        assert!(diff.statics.is_none());
        assert_eq_dynamics!(
            diff.dynamics,
            [(0, DynamicRenderJson::String("Jim".to_string()))]
        );
    }

    #[lunatic::test]
    fn template_diff_with_multiple_variables() {
        let diff = render_template_diff(
            "Hello {{ name }}, you are {{ age }} years old",
            json!({
                "name": "Bob",
                "age": 22,
            }),
            json!({
                "name": "John",
                "age": 32,
            }),
        );

        assert!(diff.statics.is_none());
        assert_eq_dynamics!(
            diff.dynamics,
            [
                (0, DynamicRenderJson::String("John".to_string())),
                (1, DynamicRenderJson::String("32".to_string()))
            ]
        );
    }

    #[lunatic::test]
    fn template_diff_with_if_statement() {
        let diff = render_template_diff(
            "Welcome {% if user %}{{ user }}{% else %}stranger{% endif %}",
            json!({
                "user": "Bob",
            }),
            json!({
                "user": Option::<&'static str>::None,
            }),
        );

        assert!(diff.statics.is_none());
        assert_eq_dynamics!(
            diff.dynamics,
            [(
                0,
                DynamicRenderJson::Nested(RenderedJson {
                    statics: Some(vec!["stranger".to_string()]),
                    dynamics: HashMap::default(),
                })
            )]
        );

        let diff = render_template_diff(
            "Welcome {% if user %}{{ user }}{% else %}stranger{% endif %}",
            json!({
                "user": Option::<&'static str>::None,
            }),
            json!({
                "user": "Bob",
            }),
        );

        assert!(diff.statics.is_none());
        assert_eq_dynamics!(
            diff.dynamics,
            [(
                0,
                DynamicRenderJson::Nested(RenderedJson {
                    statics: Some(vec!["".to_string(), "".to_string()]),
                    dynamics: HashMap::from_iter([(
                        0,
                        DynamicRenderJson::String("Bob".to_string())
                    )]),
                })
            )]
        );
    }

    #[lunatic::test]
    fn template_diff_with_nested_if_statement() {
        let diff = render_template_diff(
            r#"
                {%- if count >= 1 -%}
                    <p>Count is high!</p>
                    {%- if count >= 2 -%}
                        <p>Count is very high!</p>
                    {%- endif -%}
                {%- endif -%}
            "#,
            json!({
                "count": 0,
            }),
            json!({
                "count": 1,
            }),
        );

        assert!(diff.statics.is_none());
        assert_eq_dynamics!(
            diff.dynamics,
            [(
                0,
                DynamicRenderJson::Nested(RenderedJson {
                    statics: Some(vec!["<p>Count is high!</p>".to_string(), "".to_string()]),
                    dynamics: HashMap::from_iter([(0, DynamicRenderJson::String("".to_string()))]),
                })
            )]
        );

        let diff = render_template_diff(
            r#"
                {%- if count >= 1 -%}
                    <p>Count is high!</p>
                    {%- if count >= 2 -%}
                        <p>Count is very high!</p>
                    {%- endif -%}
                {%- endif -%}
            "#,
            json!({
                "count": 1,
            }),
            json!({
                "count": 2,
            }),
        );

        assert!(diff.statics.is_none());
        assert_eq_dynamics!(
            diff.dynamics,
            [(
                0,
                DynamicRenderJson::Nested(RenderedJson {
                    statics: None,
                    dynamics: HashMap::from_iter([(
                        0,
                        DynamicRenderJson::Nested(RenderedJson {
                            statics: Some(vec!["<p>Count is very high!</p>".to_string()]),
                            dynamics: HashMap::default()
                        })
                    )]),
                })
            )]
        );
    }
}
