use std::convert::{TryFrom, TryInto};
use std::mem;

use lunatic::process::ProcessRef;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use submillisecond::websocket::WebSocketConnection;
use thiserror::Error;

use crate::handler::{EventHandler, EventHandlerError, EventHandlerHandler};
use crate::manager::LiveViewManager;
use crate::LiveView;

/// Wrapper around a websocket connection to handle phoenix channels.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Socket<L, T>
where
    L: LiveViewManager<T>,
    T: LiveView,
{
    pub(crate) event_handler: ProcessRef<EventHandler<L, T>>,
    pub(crate) socket: RawSocket,
}

impl<L, T> Socket<L, T>
where
    L: LiveViewManager<T> + Serialize + for<'d> Deserialize<'d>,
    L::State: Clone + Serialize + for<'de> Deserialize<'de>,
    L::Reply: Serialize + for<'de> Deserialize<'de>,
    L::Error: Serialize + for<'de> Deserialize<'de>,
    T: LiveView,
{
    pub fn send_event<E>(
        &mut self,
        event: E,
    ) -> Result<(), EventHandlerError<<L as LiveViewManager<T>>::Error>>
    where
        E: Serialize,
    {
        let value = serde_json::to_value(event).map_err(|_| EventHandlerError::SerializeEvent)?;
        let reply = self.event_handler.handle_event(Event {
            name: std::any::type_name::<E>().to_string(),
            ty: "internal".to_string(),
            value,
        })?;
        self.socket
            .send(ProtocolEvent::Diff, &reply)
            .map_err(|err| EventHandlerError::SocketError(err.to_string()))
    }
}

/// Wrapper around a websocket connection to handle phoenix channels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RawSocket {
    pub(crate) conn: WebSocketConnection,
    pub(crate) ref1: Option<String>,
    pub(crate) ref2: Option<String>,
    pub(crate) topic: String,
}

impl RawSocket {
    pub fn receive(&mut self) -> Result<SocketMessage, SocketError> {
        Self::receive_from_conn(&mut self.conn)
    }

    pub fn receive_from_conn(conn: &mut WebSocketConnection) -> Result<SocketMessage, SocketError> {
        let message = conn.read_message()?;
        message.try_into()
    }

    pub fn send<T>(&mut self, event: ProtocolEvent, value: &T) -> Result<(), SocketError>
    where
        T: Serialize,
    {
        let protocol_event = serde_json::to_value(event)?;
        let text = serde_json::to_string(&json!([
            &self.ref1,
            &self.ref2,
            &self.topic,
            &protocol_event,
            value,
        ]))?;

        Ok(self.conn.write_message(tungstenite::Message::Text(text))?)
    }

    pub fn send_ok<T>(&mut self, value: &T) -> Result<(), SocketError>
    where
        T: Serialize,
    {
        self.send(
            ProtocolEvent::Reply,
            &Response {
                status: Status::Ok,
                response: value,
            },
        )
    }

    pub fn send_err<T>(&mut self, value: &T) -> Result<(), SocketError>
    where
        T: Serialize,
    {
        self.send(
            ProtocolEvent::Error,
            &Response {
                status: Status::Error,
                response: value,
            },
        )
    }
}

/// Protocol-reserved events.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolEvent {
    /// The connection will be closed.
    #[serde(rename = "phx_close")]
    Close,
    /// A tempalte diff.
    #[serde(rename = "diff")]
    Diff,
    /// A channel has errored and needs to be reconnected.
    #[serde(rename = "phx_error")]
    Error,
    /// A live view event.
    #[serde(rename = "event")]
    Event,
    /// Heartbeat.
    #[serde(rename = "heartbeat")]
    Heartbeat,
    /// Joining a channel. (Non-receivable)
    #[serde(rename = "phx_join")]
    Join,
    /// Leaving a channel. (Non-receivable)
    #[serde(rename = "phx_leave")]
    Leave,
    /// Reply to a message sent by the client.
    #[serde(rename = "phx_reply")]
    Reply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub ref1: Option<String>,
    pub ref2: Option<String>,
    pub topic: String,
    pub event: ProtocolEvent,
    pub payload: Value,
}

impl Message {
    pub fn take_event(&mut self) -> Result<Event, serde_json::Error> {
        serde_json::from_value(mem::take(&mut self.payload))
    }

    pub fn take_join_event(&mut self) -> Result<JoinEvent, serde_json::Error> {
        serde_json::from_value(mem::take(&mut self.payload))
    }

    fn to_tuple(
        &self,
    ) -> (
        &Option<String>,
        &Option<String>,
        &String,
        &ProtocolEvent,
        &Value,
    ) {
        (
            &self.ref1,
            &self.ref2,
            &self.topic,
            &self.event,
            &self.payload,
        )
    }

    fn from_tuple(
        (ref1, ref2, topic, event, payload): (
            Option<String>,
            Option<String>,
            String,
            ProtocolEvent,
            Value,
        ),
    ) -> Self {
        Message {
            ref1,
            ref2,
            topic,
            event,
            payload,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "event")]
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub value: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinEvent {
    pub url: Option<String>,
    pub redirect: Option<String>,
    pub params: JoinEventParams,
    pub session: String,
    #[serde(rename = "static")]
    pub static_token: Option<String>,
}

impl JoinEvent {
    pub fn url(&self) -> Option<&String> {
        self.url.as_ref().or(self.redirect.as_ref())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinEventParams {
    #[serde(rename = "_csrf_token")]
    pub csrf_token: String,
    #[serde(rename = "_mounts")]
    pub mounts: u32,
    #[serde(rename = "_track_static", default)]
    pub track_static: Vec<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Response<T> {
    status: Status,
    response: T,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Error,
}

pub enum SocketMessage {
    Event(Message),
    Close,
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

impl TryFrom<tungstenite::Message> for SocketMessage {
    type Error = SocketError;

    fn try_from(message: tungstenite::Message) -> Result<Self, Self::Error> {
        match message {
            tungstenite::Message::Text(text) => {
                let items = serde_json::from_str(&text)?;
                Ok(SocketMessage::Event(Message::from_tuple(items)))
            }
            tungstenite::Message::Binary(bytes) => {
                let items = serde_json::from_slice(&bytes)?;
                Ok(SocketMessage::Event(Message::from_tuple(items)))
            }
            tungstenite::Message::Ping(data) => Ok(SocketMessage::Ping(data)),
            tungstenite::Message::Pong(data) => Ok(SocketMessage::Pong(data)),
            tungstenite::Message::Close(_) => Ok(SocketMessage::Close),
            tungstenite::Message::Frame(_) => {
                unreachable!("frame should not be received with read_message");
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum SocketError {
    #[error(transparent)]
    WebsocketError(#[from] tungstenite::Error),
    #[error(transparent)]
    DeserializeError(#[from] serde_json::Error),
}
