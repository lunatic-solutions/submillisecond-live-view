use std::convert::{TryFrom, TryInto};
use std::mem;

use lunatic::{Mailbox, Process};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use submillisecond::websocket::WebSocketConnection;
use thiserror::Error;

use crate::event_handler::{EventHandler, EventHandlerError};

/// Wrapper around a websocket connection to handle phoenix channels.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Socket {
    pub(crate) event_handler: EventHandler,
    pub(crate) socket: RawSocket,
}

/// Wrapper around a websocket connection to handle phoenix channels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RawSocket {
    pub(crate) conn: WebSocketConnection,
    pub(crate) ref1: Option<String>,
    pub(crate) topic: String,
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

#[derive(Debug, Error)]
pub enum SocketError {
    #[error(transparent)]
    WebsocketError(#[from] tungstenite::Error),
    #[error(transparent)]
    DeserializeError(#[from] serde_json::Error),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Response<T> {
    status: Status,
    response: T,
}

impl Socket {
    /// Sends an event and wait for it to be sent to the socket.
    ///
    /// If you intend on sending an event from an event handler, use
    /// [`Socket::spawn_send_event`].
    pub fn send_event<E>(&mut self, event: E) -> Result<(), EventHandlerError>
    where
        E: Serialize,
    {
        Self::_send_event(event, &self.event_handler, &mut self.socket)
    }

    /// Sends an event in a spawned process.
    ///
    /// Use this if you intend to send an event from within an event handler.
    pub fn spawn_send_event<E>(&mut self, event: E)
    where
        E: Serialize + for<'de> Deserialize<'de>,
    {
        Process::spawn(
            (event, self.event_handler.clone(), self.socket.clone()),
            |(event, event_handler, mut socket), _: Mailbox<()>| {
                Self::_send_event(event, &event_handler, &mut socket).unwrap();
            },
        );
        // TODO: Use this code when <https://github.com/lunatic-solutions/lunatic-rs/pull/88> is merged and published.
        // spawn!(|event, event_handler = { self.event_handler.clone() }, socket
        // = { self.socket.clone() }| {
        // Self::_send_event(event, &event_handler, &mut socket).unwrap();
        // });
    }

    fn _send_event<E>(
        event: E,
        event_handler: &EventHandler,
        socket: &mut RawSocket,
    ) -> Result<(), EventHandlerError>
    where
        E: Serialize,
    {
        let value = serde_json::to_value(event).map_err(|_| EventHandlerError::SerializeEvent)?;
        let reply = event_handler.handle_event(Event {
            name: std::any::type_name::<E>().to_string(),
            ty: "internal".to_string(),
            value,
        })?;
        let msg = match reply {
            Some(reply) => reply,
            None => json!({}),
        };
        socket
            .send(ProtocolEvent::Diff, &msg)
            .map_err(|err| EventHandlerError::SocketError(err.to_string()))
    }
}

impl RawSocket {
    // pub fn receive(&mut self) -> Result<SocketMessage, SocketError> {
    //     Self::receive_from_conn(&mut self.conn)
    // }

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
            &None::<()>,
            &self.topic,
            &protocol_event,
            value,
        ]))?;

        Ok(self.conn.write_message(tungstenite::Message::Text(text))?)
    }

    pub fn send_reply(&mut self, message: &Message) -> Result<(), SocketError> {
        let text = serde_json::to_string(&message.to_tuple())?;
        Ok(self.conn.write_message(tungstenite::Message::Text(text))?)
    }
}

impl Message {
    pub fn reply_ok<T>(&mut self, response: T) -> &mut Self
    where
        T: Serialize,
    {
        self.event = ProtocolEvent::Reply;
        self.payload = serde_json::to_value(Response {
            status: Status::Ok,
            response,
        })
        .unwrap();
        self
    }

    pub fn reply_err<T>(&mut self, response: T) -> &mut Self
    where
        T: Serialize,
    {
        self.event = ProtocolEvent::Reply;
        self.payload = serde_json::to_value(Response {
            status: Status::Error,
            response,
        })
        .unwrap();
        self
    }

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

impl JoinEvent {
    pub fn url(&self) -> Option<&String> {
        self.url.as_ref().or(self.redirect.as_ref())
    }
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
