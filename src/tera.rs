use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

use lunatic::{abstract_process, function::FuncRef, process::ProcessRef, Process};
use lunatic_log::{error, info, warn};
use serde::{
    de::Visitor,
    ser::{SerializeMap, SerializeStruct},
    Deserialize, Serialize,
};
use serde_json::{json, Map, Value};
use submillisecond::{
    extract::FromOwnedRequest,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    websocket::WebSocket,
    Handler, RequestContext,
};
use tera::{ast::Node, Context, Error as TeraError, Tera};

use crate::{
    csrf::CsrfToken,
    socket::{ProtocolEvent, Socket, SocketError, SocketMessage},
    EventList, LiveView, LiveViewHandler,
};

pub struct LiveViewTera<T> {
    tera: Tera,
    data: PhantomData<T>,
    last_value: Option<Value>,
    not_found_handler: FuncRef<fn() -> Response>,
}

#[abstract_process]
impl<T> LiveViewTera<T>
where
    T: LiveView + Serialize + for<'de> Deserialize<'de>,
{
    #[init]
    fn init(
        _: ProcessRef<Self>,
        (path, not_found_handler): (PathBuf, FuncRef<fn() -> Response>),
    ) -> Self {
        let mut tera = Tera::default();
        tera.register_function("csrf_token", csrf_token);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("could not extract file name from template");
        tera.add_template_file(&path, Some(name))
            .expect("unable to add template file");

        println!("Here we are");

        LiveViewTera {
            tera,
            data: PhantomData::default(),
            last_value: None,
            not_found_handler,
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
            .next()
            .expect("template does not exist");
        self.tera
            .render(&name, &context)
            .map_err(|err| err.to_string())
    }

    #[handle_request]
    fn render_dynamic(&self, value: T) -> Result<Rendered, String> {
        let name = self
            .tera
            .templates
            .keys()
            .next()
            .expect("template does not exist");
        let template = self.tera.templates.get(name).unwrap();
        let context = Context::from_serialize(value).map_err(|err| err.to_string())?;
        let renderer = tera::renderer::Renderer::new(&template, &self.tera, &context);
        let mut processor = renderer.processor();

        let mut statics = Vec::new();
        let mut dynamics = Vec::new();
        let mut buffer = Vec::with_capacity(512);
        for (i, node) in template.ast.iter().enumerate() {
            let start_index = buffer.len();
            let user_defined = processor
                .render_node(node, &mut buffer)
                .map_err(|err| err.to_string())?;
            if user_defined {
                let dynamic =
                    String::from_utf8(buffer.split_off(start_index)).map_err(|error| {
                        TeraError::utf8_conversion_error(
                            error,
                            "converting node buffer to string".to_string(),
                        )
                        .to_string()
                    })?;
                let buf = std::mem::take(&mut buffer);
                let s = String::from_utf8(buf).map_err(|error| {
                    TeraError::utf8_conversion_error(
                        error,
                        "converting node buffer to string".to_string(),
                    )
                    .to_string()
                })?;
                statics.push(s);
                dynamics.push(dynamic);
            } else if i == template.ast.len() - 1 {
                let s = String::from_utf8(std::mem::take(&mut buffer)).map_err(|error| {
                    TeraError::utf8_conversion_error(
                        error,
                        "converting node buffer to string".to_string(),
                    )
                    .to_string()
                })?;
                statics.push(s);
            }
        }

        Ok(Rendered { dynamics, statics })
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
        dbg!(is_websocket);
        if is_websocket {
            // Live websocket request
            // Verify session exists
            let ws = match WebSocket::from_owned_request(req) {
                Ok(ws) => ws,
                Err(err) => return err.into_response(),
            };

            ws.on_upgrade(self.clone(), |conn, live_view| {
                let mut state: Option<(T, HashMap<usize, String>)> = None;
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

                                            let new_dynamics: HashMap<usize, String> = live_view
                                                .render_dynamic(state.clone())
                                                .expect("failed to render template")
                                                .dynamics
                                                .into_iter()
                                                .enumerate()
                                                .collect();

                                            let result: HashMap<&usize, &String> = new_dynamics
                                                .iter()
                                                .filter(|(i, value)| match prev_dynamics.get(i) {
                                                    Some(prev_value) => prev_value != *value,
                                                    None => true,
                                                })
                                                .collect();

                                            socket
                                                .send(message.reply_ok(json!({ "diff": result })))
                                                .unwrap();

                                            *prev_dynamics = new_dynamics;
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
                                    state = Some((
                                        mount_state,
                                        rendered.dynamics.clone().into_iter().enumerate().collect(),
                                    ));
                                    socket.send(message.reply_ok(rendered)).unwrap();
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

#[derive(Clone, Debug)]
struct Rendered {
    dynamics: Vec<String>,
    statics: Vec<String>,
}

impl Serialize for Rendered {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        struct RenderedInner<'a> {
            dynamics: &'a [String],
            statics: &'a [String],
        }

        impl Serialize for RenderedInner<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut map = serializer.serialize_map(Some(self.dynamics.len() + 1))?;
                map.serialize_entry("s", &self.statics)?;
                for (i, value) in self.dynamics.iter().enumerate() {
                    map.serialize_entry(&i, value)?;
                }
                map.end()
            }
        }

        let mut s = serializer.serialize_struct("rendered", 1)?;
        s.serialize_field(
            "rendered",
            &RenderedInner {
                dynamics: &self.dynamics,
                statics: &self.statics,
            },
        )?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for Rendered {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RenderedMap {
            rendered: Rendered,
        }

        impl<'de> Deserialize<'de> for RenderedMap {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
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
                            std::iter::from_fn(|| map.next_entry::<usize, String>().transpose())
                                .map(|item| item.map(|(_, value)| value))
                                .collect::<Result<_, _>>()?;

                        Ok(RenderedMap {
                            rendered: Rendered { dynamics, statics },
                        })
                    }

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(formatter, "a map of dynamic values and a static array")
                    }
                }

                deserializer.deserialize_map(RenderedMapVisitor)
            }
        }

        #[derive(Deserialize)]
        struct RenderedWrapper {
            rendered: RenderedMap,
        }

        let rendered_outer = RenderedWrapper::deserialize(deserializer)?;
        Ok(rendered_outer.rendered.rendered)
    }
}

fn node_is_static(node: &Node) -> bool {
    matches!(node, Node::Text(_) | Node::Raw(_, _, _))
}
