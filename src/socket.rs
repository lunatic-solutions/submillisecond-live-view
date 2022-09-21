use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use submillisecond::websocket::WebSocketConnection;
use thiserror::Error;

/// Wrapper around a websocket connection to handle phoenix channels.
pub struct Socket {
    conn: WebSocketConnection,
}

impl Socket {
    pub fn new(conn: WebSocketConnection) -> Self {
        Socket { conn }
    }

    pub fn receive(&mut self) -> Result<SocketMessage, SocketError> {
        let message = self.conn.read_message()?;
        message.try_into()
    }

    pub fn send(&mut self, event: &Message) -> Result<(), SocketError> {
        self.conn
            .write_message(tungstenite::Message::Text(serde_json::to_string(
                &event.to_tuple(),
            )?))?;
        Ok(())
    }
}

/// Protocol-reserved events.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolEvent {
    /// The connection will be closed.
    #[serde(rename = "phx_close")]
    Close,
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

    pub fn as_event(&self) -> Result<Event, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
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

// #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
// pub struct EventValue(HashMap<String, String>);

// impl EventValue {
//     pub fn value(&self) -> Option<&String> {
//         self.get("value")
//     }
// }

// impl Deref for EventValue {
//     type Target = HashMap<String, String>;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl DerefMut for EventValue {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Response<T> {
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
    ReadMessageFailed(#[from] tungstenite::Error),
    #[error(transparent)]
    DeserializeError(#[from] serde_json::Error),
}
