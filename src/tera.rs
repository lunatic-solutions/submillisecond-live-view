use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
    path::Path,
};

use lunatic::process_local;
use lunatic_log::{info, warn};
use once_cell::sync::OnceCell;
use rand::prelude::*;
use serde::{
    de::Visitor,
    ser::{SerializeMap, SerializeStruct, SerializeTuple},
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
    LiveView,
};

pub struct LiveViewTera<T> {
    tera: Tera,
    data: PhantomData<T>,
    last_value: Option<Value>,
    not_found_handler: fn() -> Response,
}

impl<T> LiveViewTera<T>
where
    T: LiveView,
{
    pub fn new(path: &str, not_found_handler: fn() -> Response) -> Result<Self, TeraError> {
        let mut tera = Tera::default();
        tera.register_function("csrf_token", csrf_token);

        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("could not extract file name from template");
        tera.add_template_file(path, Some(name))?;

        Ok(LiveViewTera {
            tera,
            data: PhantomData::default(),
            last_value: None,
            not_found_handler,
        })
    }

    fn render_dynamic(&self, name: &str, values: Value) -> Result<Rendered, TeraError> {
        let template = self.tera.templates.get(name).unwrap();
        let mut template_clone = template.clone();
        let template_sections = template.ast.split_inclusive(|node| !node_is_static(node));
        let mut statics = Vec::new();
        let mut dynamics = Vec::new();
        let context = Context::from_value(values)?;
        let renderer = tera::renderer::Renderer::new(&template, &self.tera, &context);
        let mut processor = renderer.processor();
        let mut buffer = Vec::with_capacity(512);
        for (i, node) in template.ast.iter().enumerate() {
            let start_index = buffer.len();
            let user_defined = processor.render_node(node, &mut buffer)?;
            if user_defined {
                let dynamic =
                    String::from_utf8(buffer.split_off(start_index)).map_err(|error| {
                        TeraError::utf8_conversion_error(
                            error,
                            "converting node buffer to string".to_string(),
                        )
                    })?;
                let buf = std::mem::take(&mut buffer);
                let s = String::from_utf8(buf).map_err(|error| {
                    TeraError::utf8_conversion_error(
                        error,
                        "converting node buffer to string".to_string(),
                    )
                })?;
                statics.push(s);
                dynamics.push(dynamic);
            } else if i == template.ast.len() - 1 {
                let s = String::from_utf8(std::mem::take(&mut buffer)).map_err(|error| {
                    TeraError::utf8_conversion_error(
                        error,
                        "converting node buffer to string".to_string(),
                    )
                })?;
                statics.push(s);
            }
        }

        // dbg!(&template.ast);
        // for nodes in template_sections {
        //     let last_node_is_static = nodes.last().map(node_is_static).unwrap_or(false);
        //     let (dynamic, nodes) = if last_node_is_static {
        //         (None, nodes)
        //     } else {
        //         match nodes.split_last() {
        //             Some((dynamic, nodes)) => (Some(dynamic), nodes),
        //             None => (None, &[] as &[Node]),
        //         }
        //     };

        //     // dbg!(last_node_is_static);
        //     // let (dynamic, nodes) = if nodes.len() > 1 {
        //     //     match nodes.split_last() {
        //     //         Some((dynamic, nodes)) => (Some(dynamic), nodes),
        //     //         None => (None, &[] as &[Node]),
        //     //     }
        //     // } else {
        //     //     (nodes.get(0), &[] as &[Node])
        //     // };

        //     template_clone.ast = nodes.to_vec();
        //     let renderer = tera::renderer::Renderer::new(&template_clone, &self.tera, &context);
        //     statics.push(renderer.render()?);

        //     if let Some(dynamic) = dynamic {
        //         let mut buffer = Vec::new();
        //         renderer.processor().render_node(dynamic, &mut buffer)?;
        //         let value = String::from_utf8(buffer).map_err(|error| {
        //             TeraError::utf8_conversion_error(
        //                 error,
        //                 "converting node buffer to string".to_string(),
        //             )
        //         })?;
        //         dynamics.push(value);
        //     }
        // }

        // dbg!(&dynamics);
        // dbg!(&statics);
        // let renderer = tera::renderer::Renderer::new(
        //     &template_clone,
        //     &self.tera,
        //     &Context::from_value(values)?,
        // );
        // let pp: Vec<_> = template
        //     .ast
        //     .split(|node| matches!(node, tera::ast::Node::VariableBlock(_, _)))
        //     .collect();

        // let mut placeholder_values = values.clone();
        // let dynamics = match placeholder_values {
        //     Value::Object(ref mut map) => map.values_mut().fold(Vec::new(), |mut acc, item| {
        //         acc.push(item.to_string());
        //         *item = Value::String((0x0 as char).to_string());
        //         acc
        //     }),
        //     _ => {
        //         return Err(tera::Error::msg(
        //             "Creating a Context from a Value/Serialize requires it being a JSON object",
        //         ))
        //     }
        // };
        // let rendered = self
        //     .tera
        //     .render(name, &Context::from_value(placeholder_values)?)?;
        // let statics = rendered
        //     .split(0x0 as char)
        //     .into_iter()
        //     .map(|s| s.to_string())
        //     .collect();

        Ok(Rendered { dynamics, statics })
    }

    fn render_static(&self, name: &str, context: &Context) -> Result<String, TeraError> {
        self.tera.render(name, context)
    }
}

impl<T> Handler for LiveViewTera<T>
where
    T: LiveView,
{
    fn handle(&self, req: RequestContext) -> Response {
        if *req.method() != ::submillisecond::http::Method::GET {
            return (self.not_found_handler)();
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

            let name = self
                .tera
                .get_template_names()
                .next()
                .expect("at least one template should have been loaded");
            let rendered = self
                .render_dynamic(
                    name,
                    json!({
                        "name": "Ari",
                        "age": 22,
                    }),
                )
                .unwrap();
            dbg!(&rendered);

            ws.on_upgrade(rendered, |conn, rendered| {
                let mut socket = Socket::new(conn);
                loop {
                    match socket.receive() {
                        Ok(SocketMessage::Event(mut event)) => {
                            info!("Received event: {event:?}");
                            match event.event {
                                ProtocolEvent::Close => {
                                    info!("Client left");
                                    break;
                                }
                                ProtocolEvent::Error => {}
                                ProtocolEvent::Event => {
                                    // event.p
                                }
                                ProtocolEvent::Heartbeat => {
                                    socket.send(event.reply_ok(Map::default())).unwrap();
                                }
                                ProtocolEvent::Join => {
                                    socket.send(event.reply_ok(rendered.clone())).unwrap();
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
            // Static GET request
            // let name = req.reader.read_to_end();
            if !req.reader.is_dangling_slash() {
                return (self.not_found_handler)();
            }

            let name = self
                .tera
                .get_template_names()
                .next()
                .expect("at least one template should have been loaded");
            match self.render_static(
                name,
                &Context::from_value(json!({
                    "name": "Ari",
                    "age": 22,
                }))
                .unwrap(),
            ) {
                Ok(body) => Response::builder()
                    .header("Content-Type", "text/html; charset=UTF-8")
                    .body(body.into_bytes())
                    .unwrap(),
                Err(err) => match err.kind {
                    tera::ErrorKind::TemplateNotFound(_) => (self.not_found_handler)(),
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
                },
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
