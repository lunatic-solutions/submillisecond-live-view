//! Handler functionality for handling LiveViews.

use std::fmt;
use std::marker::PhantomData;

use lunatic_log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use submillisecond::extract::FromOwnedRequest;
use submillisecond::http::header;
use submillisecond::response::{IntoResponse, Response};
use submillisecond::websocket::{WebSocket, WebSocketConnection};
use submillisecond::{Handler, RequestContext};

use crate::event_handler::EventHandler;
use crate::manager::LiveViewManager;
use crate::maud::LiveViewMaud;
use crate::socket::{Message, ProtocolEvent, RawSocket, SocketError, SocketMessage};
use crate::template::TemplateProcess;
use crate::LiveView;

type Manager<T> = LiveViewMaud<T>;

/// A LiveView handler created with `LiveViewRouter::handler`.
pub struct LiveViewHandler<L, T> {
    live_view: L,
    phantom: PhantomData<T>,
}

/// Trait used to create a handler from a LiveView.
pub trait LiveViewRouter: Sized {
    /// Create handler for LiveView with a html template.
    ///
    /// The LiveView is injected into the selector of the template.
    ///
    /// # Example
    ///
    /// ```
    /// router! {
    ///     GET "/" => MyLiveView::handler("index.html", "#app")
    /// }
    /// ```
    fn handler(template: &str, selector: &str) -> LiveViewHandler<Manager<Self>, Self>;
}

trait LogError {
    fn log_warn(self);
    fn log_error(self);
}

impl<T> LiveViewRouter for T
where
    T: LiveView,
{
    fn handler(template: &str, selector: &str) -> LiveViewHandler<Manager<Self>, Self> {
        // TODO lookup_or_start could result in a race condition. Need to solve this
        // somehow.
        let process = TemplateProcess::lookup_or_start(template, selector)
            .expect("failed to load index.html");

        LiveViewHandler::new(Manager::new(process))
    }
}

impl<L, T> LiveViewHandler<L, T> {
    pub(crate) fn new(live_view: L) -> Self {
        LiveViewHandler {
            live_view,
            phantom: PhantomData,
        }
    }
}

impl<L, T> Handler for LiveViewHandler<L, T>
where
    L: Clone + LiveViewManager<T> + Serialize + for<'de> Deserialize<'de>,
    // L::Reply: Serialize + for<'de> Deserialize<'de>,
    L::Error: Serialize + for<'de> Deserialize<'de>,
    T: LiveView,
{
    fn handle(&self, req: RequestContext) -> Response {
        let is_websocket = req
            .headers()
            .get(header::UPGRADE)
            .and_then(|upgrade| upgrade.to_str().ok())
            .map(|upgrade| upgrade == "websocket")
            .unwrap_or(false);
        if is_websocket {
            let ws = match WebSocket::from_owned_request(req) {
                Ok(ws) => ws,
                Err(err) => return err.into_response(),
            };

            ws.on_upgrade(self.live_view.clone(), |conn, live_view| {
                let (mut socket, mut message) = match wait_for_join(conn) {
                    Ok((socket, message)) => (socket, message),
                    Err(err) => {
                        error!("{err}");
                        return;
                    },
                };
                let mut conn = socket.conn.clone();
                let event_handler = EventHandler::spawn(socket.clone(), live_view);

                match event_handler.handle_join(message.take_join_event().unwrap()) {
                    Ok(reply) => {
                        socket.send_reply(message.reply_ok(json!({ "rendered": reply }))).unwrap();
                    }
                    Err(err) => {
                        error!("{err}");
                        return
                    }
                }

                loop {
                    match RawSocket::receive_from_conn(&mut conn) {
                        Ok(SocketMessage::Event(message)) => {
                            if !handle_message::<L, T>(&mut socket, message, &event_handler) {
                                break;
                            }
                        }
                        Ok(SocketMessage::Ping(_)) |
                        Ok(SocketMessage::Pong(_)) => {}
                        Ok(SocketMessage::Close) => {
                            info!("Socket connection closed");
                            break;
                        }
                        Err(SocketError::WebsocketError(tungstenite::Error::AlreadyClosed))
                        | Err(SocketError::WebsocketError(
                            tungstenite::Error::ConnectionClosed,
                        )) => {
                            info!("connection closed");
                            break;
                        }
                        Err(SocketError::WebsocketError(err)) => {
                            warn!("read message failed: {err}");
                            break;
                        }
                        Err(SocketError::DeserializeError(err)) => {
                            warn!("deserialization failed: {err}");
                        }
                    }
                }
            })
            .into_response()
        } else {
            self.live_view.handle_request(req)
        }
    }
}

fn wait_for_join(mut conn: WebSocketConnection) -> Result<(RawSocket, Message), SocketError> {
    loop {
        match RawSocket::receive_from_conn(&mut conn) {
            Ok(SocketMessage::Event(
                message @ Message {
                    event: ProtocolEvent::Join,
                    ..
                },
            )) => {
                return Ok((
                    RawSocket {
                        conn,
                        ref1: message.ref1.clone(),
                        topic: message.topic.clone(),
                    },
                    message,
                ));
            }
            Ok(SocketMessage::Event(Message {
                event: ProtocolEvent::Close,
                ..
            }))
            | Ok(SocketMessage::Event(Message {
                event: ProtocolEvent::Leave,
                ..
            }))
            | Ok(SocketMessage::Close) => {
                return Err(SocketError::WebsocketError(
                    tungstenite::Error::ConnectionClosed,
                ));
            }
            Ok(SocketMessage::Event(_) | SocketMessage::Ping(_) | SocketMessage::Pong(_)) => {}
            Err(SocketError::WebsocketError(err @ tungstenite::Error::AlreadyClosed))
            | Err(SocketError::WebsocketError(err @ tungstenite::Error::ConnectionClosed))
            | Err(SocketError::WebsocketError(err)) => {
                return Err(SocketError::WebsocketError(err));
            }
            Err(SocketError::DeserializeError(err)) => {
                warn!("deserialization failed: {err}");
            }
        }
    }
}

fn handle_message<L, T>(
    socket: &mut RawSocket,
    mut message: Message,
    event_handler: &EventHandler,
) -> bool
where
    L: LiveViewManager<T> + Serialize + for<'de> Deserialize<'de>,
    // L::Reply: Serialize + for<'de> Deserialize<'de>,
    L::Error: Serialize + for<'de> Deserialize<'de>,
    T: LiveView,
{
    trace!("Received message: {message:?}");
    match message.event {
        ProtocolEvent::Close => {
            info!("Client left");
            false
        }
        ProtocolEvent::Diff => true,
        ProtocolEvent::Error => true,
        ProtocolEvent::Event => match message.take_event() {
            Ok(event) => {
                info!("Received event {}", event.name);
                match event_handler.handle_event(event) {
                    Ok(Some(reply)) => {
                        socket
                            .send_reply(message.reply_ok(json!({ "diff": reply })))
                            .log_warn();
                    }
                    Ok(None) => {
                        socket.send_reply(message.reply_ok(json!({}))).log_warn();
                    }
                    Err(err) => {
                        error!("{err}");
                    }
                }
                true
            }
            Err(err) => {
                error!("{err}");
                true
            }
        },
        ProtocolEvent::Heartbeat => {
            socket.send_reply(message.reply_ok(json!({}))).log_error();
            true
        }
        ProtocolEvent::Join => false,
        ProtocolEvent::Leave => {
            info!("Client left");
            false
        }
        ProtocolEvent::Reply => true,
    }
}

impl<E> LogError for Result<(), E>
where
    E: fmt::Display,
{
    fn log_warn(self) {
        if let Err(err) = self {
            warn!("{err}");
        }
    }

    fn log_error(self) {
        if let Err(err) = self {
            error!("{err}");
        }
    }
}
