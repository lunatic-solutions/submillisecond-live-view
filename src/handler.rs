use std::fmt;
use std::marker::PhantomData;

use lunatic::abstract_process;
use lunatic::process::{ProcessRef, StartProcess};
use lunatic_log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map};
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
    L::Reply: Serialize + for<'de> Deserialize<'de>,
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
                let (mut socket, join_event) = match wait_for_join(conn) {
                    Ok((socket, join_event)) => (socket, join_event),
                    Err(err) => {
                        error!("{err}");
                        return;
                    },
                };
                let mut conn = socket.conn.clone();
                let event_handler = EventHandler::<L, T>::start_link((socket.clone(), live_view), None);

                match event_handler.handle_join(join_event) {
                    Ok(reply) => {
                        socket.send_ok(&json!({ "rendered": reply })).unwrap();
                    }
                    Err(err) => {
                        error!("{err}");
                        return
                    }
                }

                loop {
                    match RawSocket::receive_from_conn(&mut conn) {
                        Ok(SocketMessage::Event(message)) => {
                            if !handle_message(&mut socket, message, &event_handler) {
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

fn wait_for_join(mut conn: WebSocketConnection) -> Result<(RawSocket, JoinEvent), SocketError> {
    loop {
        match RawSocket::receive_from_conn(&mut conn) {
            Ok(SocketMessage::Event(
                mut message @ Message {
                    event: ProtocolEvent::Join,
                    ..
                },
            )) => {
                println!("Taking join event");
                let join_event = message.take_join_event()?;
                println!("Took");
                return Ok((
                    RawSocket {
                        conn,
                        ref1: message.ref1,
                        ref2: message.ref2,
                        topic: message.topic,
                    },
                    join_event,
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

#[derive(Serialize, Deserialize)]
pub(crate) struct EventHandler<L, T>
where
    L: LiveViewManager<T>,
    T: LiveView,
{
    this: ProcessRef<Self>,
    state: Option<(T, L::State)>,
    socket: RawSocket,
    manager: L,
}

#[abstract_process(visibility = pub(crate))]
impl<L, T> EventHandler<L, T>
where
    L: LiveViewManager<T> + Serialize + for<'de> Deserialize<'de>,
    L::Reply: Serialize + for<'de> Deserialize<'de>,
    L::Error: Serialize + for<'de> Deserialize<'de>,
    T: LiveView,
{
    #[init]
    fn init(this: ProcessRef<Self>, (socket, manager): (RawSocket, L)) -> Self {
        EventHandler {
            this,
            state: None,
            socket,
            manager,
        }
    }

    #[handle_request]
    fn handle_join(
        &mut self,
        join_event: JoinEvent,
    ) -> Result<L::Reply, EventHandlerError<L::Error>> {
        match self
            .manager
            .handle_join(
                Socket {
                    event_handler: self.this.clone(),
                    socket: self.socket.clone(),
                },
                join_event,
            )
            .into_result()
            .map_err(EventHandlerError::ManagerError)
        {
            Ok(Join {
                live_view,
                state,
                reply,
            }) => {
                self.state = Some((live_view, state));
                Ok(reply)
            }
            Err(err) => Err(err),
        }
    }

    #[handle_request]
    fn handle_event(
        &mut self,
        event: Event,
    ) -> Result<Option<L::Reply>, EventHandlerError<L::Error>> {
        match &mut self.state {
            Some((live_view, state)) => {
                match <T::Events as EventList<T>>::handle_event(live_view, event.clone()) {
                    Ok(handled) => {
                        if !handled {
                            Err(EventHandlerError::UnknownEvent)
                        } else {
                            let reply = self
                                .manager
                                .handle_event(event, state, live_view)
                                .into_result()
                                .map_err(EventHandlerError::ManagerError)?;

                            Ok(reply)
                        }
                    }
                    Err(_) => Err(EventHandlerError::DeserializeEvent),
                }
            }
            None => Err(EventHandlerError::NotMounted),
        }
    }
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum EventHandlerError<M> {
    #[error("deserialize event failed")]
    DeserializeEvent,
    #[error("serialize event failed")]
    SerializeEvent,
    #[error(transparent)]
    ManagerError(M),
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
    event_handler: &ProcessRef<EventHandler<L, T>>,
) -> bool
where
    L: LiveViewManager<T> + Serialize + for<'de> Deserialize<'de>,
    L::Reply: Serialize + for<'de> Deserialize<'de>,
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
                        socket.send_ok(&json!({ "diff": reply })).log_warn();
                    }
                    Ok(None) => {
                        socket.send_ok(&json!({})).log_warn();
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
            socket.send_ok(&Map::default()).log_error();
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
