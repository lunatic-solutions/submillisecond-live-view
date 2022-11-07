use std::fmt;
use std::marker::PhantomData;

use lunatic::serializer::Json;
use lunatic::{Mailbox, Process, Tag};
use lunatic_log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use submillisecond::extract::FromOwnedRequest;
use submillisecond::http::header;
use submillisecond::response::{IntoResponse, Response};
use submillisecond::websocket::{WebSocket, WebSocketConnection};
use submillisecond::{Handler, RequestContext};
use thiserror::Error;

use crate::manager::{Join, LiveViewManager};
use crate::maud::LiveViewMaud;
use crate::socket::{
    Event, JoinEvent, Message, ProtocolEvent, RawSocket, Socket, SocketError, SocketMessage,
};
use crate::{EventList, LiveView};

type Manager<T> = LiveViewMaud<T>;

pub trait LiveViewRouter: Sized {
    fn handler() -> LiveViewHandler<Manager<Self>, Self>;
}

impl<T> LiveViewRouter for T
where
    T: LiveView,
{
    fn handler() -> LiveViewHandler<Manager<Self>, Self> {
        LiveViewHandler::new(Manager::default())
    }
}

pub struct LiveViewHandler<L, T> {
    live_view: L,
    phantom: PhantomData<T>,
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
            let ws = match WebSocket::from_owned_request(req) {
                Ok(ws) => ws,
                Err(err) => return err.into_response(),
            };

            ws.on_upgrade(self.live_view.clone(), |conn, live_view| {
                println!("Waiting for join");
                let (mut socket, mut message) = match wait_for_join(conn) {
                    Ok((socket, message)) => (socket, message),
                    Err(err) => {
                        error!("{err}");
                        return;
                    },
                };
                let mut conn = socket.conn.clone();
                let event_handler = EventHandler { event_handler: Process::spawn_link((socket.clone(), live_view), event_handler) };

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
            if !req.reader.is_dangling_slash() {
                return T::not_found(req);
            }

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum EventHandlerMessage {
    HandleJoin(
        Process<Result<Value, EventHandlerError>, Json>,
        Tag,
        JoinEvent,
    ),
    HandleEvent(
        Process<Result<Option<Value>, EventHandlerError>, Json>,
        Tag,
        Event,
    ),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EventHandler {
    event_handler: Process<EventHandlerMessage, Json>,
}

impl EventHandler {
    pub(crate) fn handle_join(&self, join_event: JoinEvent) -> Result<Value, EventHandlerError> {
        let tag = Tag::new();
        self.event_handler.send(EventHandlerMessage::HandleJoin(
            Process::this(),
            tag,
            join_event,
        ));
        let mailbox: Mailbox<Result<Value, EventHandlerError>, Json> = unsafe { Mailbox::new() };
        mailbox.tag_receive(&[tag])
    }

    pub(crate) fn handle_event(&self, event: Event) -> Result<Option<Value>, EventHandlerError> {
        let tag = Tag::new();
        self.event_handler.send(EventHandlerMessage::HandleEvent(
            Process::this(),
            tag,
            event,
        ));
        let mailbox: Mailbox<Result<Option<Value>, EventHandlerError>, Json> =
            unsafe { Mailbox::new() };
        mailbox.tag_receive(&[tag])
    }
}

fn event_handler<L, T>(
    (socket, manager): (RawSocket, L),
    mailbox: Mailbox<EventHandlerMessage, Json>,
) where
    L: LiveViewManager<T>,
    T: LiveView,
{
    let this: Process<EventHandlerMessage, Json> = Process::this();
    let mut state = None;

    loop {
        let message = mailbox.receive();
        match message {
            EventHandlerMessage::HandleJoin(parent, tag, join_event) => {
                let reply = match manager
                    .handle_join(
                        Socket {
                            event_handler: EventHandler {
                                event_handler: this.clone(),
                            },
                            socket: socket.clone(),
                        },
                        join_event,
                    )
                    .into_result()
                {
                    Ok(Join {
                        live_view,
                        state: new_state,
                        reply,
                    }) => {
                        state = Some((live_view, new_state));
                        Ok(reply)
                    }
                    Err(err) => Err(EventHandlerError::ManagerError(err.to_string())),
                };
                parent.tag_send(tag, reply);
            }
            EventHandlerMessage::HandleEvent(parent, tag, event) => {
                let reply = match &mut state {
                    Some((live_view, state)) => {
                        match <T::Events as EventList<T>>::handle_event(live_view, event.clone()) {
                            Ok(handled) => {
                                if !handled {
                                    Err(EventHandlerError::UnknownEvent)
                                } else {
                                    manager
                                        .handle_event(event, state, live_view)
                                        .into_result()
                                        .map_err(|err| {
                                            EventHandlerError::ManagerError(err.to_string())
                                        })
                                }
                            }
                            Err(_) => Err(EventHandlerError::DeserializeEvent),
                        }
                    }
                    None => Err(EventHandlerError::NotMounted),
                };
                parent.tag_send(tag, reply);
            }
        };
    }
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum EventHandlerError {
    #[error("deserialize event failed")]
    DeserializeEvent,
    #[error("serialize event failed")]
    SerializeEvent,
    #[error("manager error: {0}")]
    ManagerError(String),
    #[error("not mounted")]
    NotMounted,
    #[error("socket error: {0}")]
    SocketError(String),
    #[error("unknown event")]
    UnknownEvent,
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

trait LogError {
    fn log_warn(self);
    fn log_error(self);
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
