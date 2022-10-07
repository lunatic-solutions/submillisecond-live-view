mod json;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;

use lunatic::abstract_process;
use lunatic::process::ProcessRef;
use lunatic_log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use submillisecond::extract::FromOwnedRequest;
use submillisecond::http::{header, StatusCode};
use submillisecond::response::{IntoResponse, Response};
use submillisecond::websocket::WebSocket;
use submillisecond::RequestContext;
use tera::{Context, RenderVisitor, Tera};

use crate::csrf::CsrfToken;
use crate::socket::{ProtocolEvent, Socket, SocketError, SocketMessage};
use crate::tera::json::RenderedJson;
use crate::{EventList, LiveView, LiveViewHandler};

pub struct LiveViewTera<T> {
    tera: Tera,
    data: PhantomData<T>,
}

#[abstract_process]
impl<T> LiveViewTera<T>
where
    T: LiveView + Serialize + for<'de> Deserialize<'de>,
{
    #[init]
    fn init(_: ProcessRef<Self>, (layout, path): (PathBuf, PathBuf)) -> Self {
        let mut tera = Tera::default();
        tera.register_function("csrf_token", csrf_token);
        tera.autoescape_on(vec![]);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("could not extract file name from template");
        tera.add_template_file(layout, Some("__layout"))
            .expect("unable to add layout file");
        tera.add_template_file(&path, Some(name))
            .expect("unable to add template file");

        LiveViewTera {
            tera,
            data: PhantomData::default(),
        }
    }

    #[terminate]
    fn terminate(self) {
        println!("Shutdown process");
    }

    #[handle_request]
    fn render_static(&self, value: T) -> Result<String, String> {
        let context = Context::from_serialize(value).map_err(|err| err.to_string())?;
        let name = self
            .tera
            .templates
            .keys()
            .find(|name| *name != "__layout")
            .expect("template does not exist");
        let content = self
            .tera
            .render(name, &context)
            .map_err(|err| err.to_string())?;

        let mut context = Context::new();
        context.insert(
            "inner_content",
            &format!(
                r#"
                <div
                    data-phx-main="true"
                    data-phx-session="session"
                    data-phx-static="static"
                    id="phx-FxSmBHsfHn_3LQAD"
                    class="phx-connected"
                    data-phx-root-id="phx-FxSmBHsfHn_3LQAD"
                >
                    {content}
                </div>"#
            ),
        );
        self.tera
            .render("__layout", &context)
            .map_err(|err| err.to_string())
    }

    #[handle_request]
    fn render_dynamic(&self, value: T) -> Result<Rendered, String> {
        let context = Context::from_serialize(value).map_err(|err| err.to_string())?;
        let name = self
            .tera
            .templates
            .keys()
            .find(|name| *name != "__layout")
            .expect("template does not exist");
        let mut rendered = Rendered::default();
        self.tera
            .render_to(name, &context, &mut rendered)
            .map_err(|err| err.to_string())?;

        Ok(rendered)
    }
}

impl<T> LiveViewHandler for ProcessRef<LiveViewTera<T>>
where
    T: Clone + LiveView + Serialize + for<'de> Deserialize<'de>,
{
    fn handle(&self, req: RequestContext) -> Response {
        if *req.method() != ::submillisecond::http::Method::GET {
            return T::not_found(req);
        }

        let is_websocket = req
            .headers()
            .get(header::UPGRADE)
            .and_then(|upgrade| upgrade.to_str().ok())
            .map(|upgrade| upgrade == "websocket")
            .unwrap_or(false);
        if is_websocket {
            // Live websocket request
            // Verify session exists
            let ws = match WebSocket::from_owned_request(req) {
                Ok(ws) => ws,
                Err(err) => return err.into_response(),
            };

            ws.on_upgrade(self.clone(), |conn, live_view| {
                let mut state: Option<(T, RenderedJson)> = None;
                let mut socket = Socket::new(conn);
                loop {
                    match socket.receive() {
                        Ok(SocketMessage::Event(mut message)) => {
                            info!("Received message: {message:?}");
                            match message.event {
                                ProtocolEvent::Close => {
                                    info!("Client left");
                                    break;
                                }
                                ProtocolEvent::Error => {}
                                ProtocolEvent::Event => match message.as_event() {
                                    Ok(event) => match state.as_mut() {
                                        Some((state, prev_dynamics)) => {
                                            match <T::Events as EventList<T>>::handle_event(
                                                state, event,
                                            ) {
                                                Ok(handled) => {
                                                    if !handled {
                                                        warn!("received unknown event");
                                                        continue;
                                                    }
                                                }
                                                Err(err) => {
                                                    warn!("failed to deserialize event: {err}");
                                                    continue;
                                                }
                                            }

                                            let rendered = RenderedJson::from(
                                                live_view
                                                    .render_dynamic(state.clone())
                                                    .expect("failed to render template"),
                                            );

                                            let diff = prev_dynamics.diff(&rendered);
                                            *prev_dynamics = rendered;

                                            socket
                                                .send(message.reply_ok(json!({ "diff": diff })))
                                                .unwrap();
                                        }
                                        None => {
                                            warn!("event received before mount");
                                            continue;
                                        }
                                    },
                                    Err(err) => {
                                        error!("{err}");
                                        continue;
                                    }
                                },
                                ProtocolEvent::Heartbeat => {
                                    socket.send(message.reply_ok(Map::default())).unwrap();
                                }
                                ProtocolEvent::Join => {
                                    let mount_state = T::mount(Some(&socket));
                                    let rendered = live_view
                                        .render_dynamic(mount_state.clone())
                                        .expect("failed to render template");
                                    state =
                                        Some((mount_state, RenderedJson::from(rendered.clone())));
                                    socket
                                        .send(message.reply_ok(json!({
                                            "rendered": RenderedJson::from(rendered)
                                        })))
                                        .unwrap();
                                }
                                ProtocolEvent::Leave => {
                                    info!("Client left");
                                    break;
                                }
                                ProtocolEvent::Reply => {}
                            }
                        }
                        Ok(SocketMessage::Ping(_) | SocketMessage::Pong(_)) => {}
                        Ok(SocketMessage::Close) => {
                            info!("Socket connection closed");
                            break;
                        }
                        Err(SocketError::ReadMessageFailed(err)) => {
                            warn!("Read message failed: {err}");
                            break;
                        }
                        Err(SocketError::DeserializeError(err)) => {
                            warn!("Deserialization failed: {err}");
                        }
                    }
                }
            })
            .into_response()
        } else {
            if !req.reader.is_dangling_slash() {
                return T::not_found(req);
            }

            match self.render_static(T::mount(None)) {
                Ok(body) => Response::builder()
                    .header("Content-Type", "text/html; charset=UTF-8")
                    .body(body.into_bytes())
                    .unwrap(),
                Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
            }
        }
    }
}

fn csrf_token(_: &HashMap<String, Value>) -> tera::Result<Value> {
    Ok(CsrfToken::get_or_init().masked.clone().into())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    statics: Vec<String>,
    dynamics: Vec<DynamicRender>,
    nested: bool,
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

    // fn last_parent_mut(&mut self) -> Option<&mut Self> {
    //     if !self.nested {
    //         return None;
    //     }

    //     let mut current = self;
    //     loop {
    //         let next = match current.dynamics.last_mut().unwrap() {
    //             DynamicRender::String(_) => unreachable!(),
    //             DynamicRender::Nested(nested) => nested,
    //         };
    //         if !next.nested {
    //             return Some(current);
    //         }
    //         current = match current.dynamics.last_mut().unwrap() {
    //             DynamicRender::String(_) => unreachable!(),
    //             DynamicRender::Nested(nested) => nested,
    //         };
    //     }
    // }
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

    use super::json::RenderedJson;
    use super::Rendered;
    use crate::tera::json::DynamicRenderJson;

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

    use super::json::RenderedJson;
    use super::Rendered;
    use crate::tera::json::DynamicRenderJson;

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
