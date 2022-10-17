use std::convert::{TryFrom, TryInto};
use std::mem;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::distributions::Alphanumeric;
use rand::rngs::ThreadRng;
use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use submillisecond::websocket::WebSocketConnection;
use thiserror::Error;

pub struct SocketBuilder {
    id: String,
    conn: WebSocketConnection,
    vsn: u128,
    rng: ThreadRng,
}

impl SocketBuilder {
    pub fn generate_secret_key(mut self) -> Socket {
        let mut secret_key = [0; 64];
        self.rng.fill_bytes(&mut secret_key);

        Socket {
            id: self.id,
            conn: self.conn,
            secret_key,
            vsn: self.vsn,
        }
    }

    pub fn secret_key(self, secret_key: [u8; 64]) -> Socket {
        Socket {
            id: self.id,
            conn: self.conn,
            secret_key,
            vsn: self.vsn,
        }
    }
}

/// Wrapper around a websocket connection to handle phoenix channels.
pub struct Socket {
    id: String,
    conn: WebSocketConnection,
    secret_key: [u8; 64],
    vsn: u128,
}

impl Socket {
    pub fn builder(conn: WebSocketConnection) -> SocketBuilder {
        let mut rng = rand::thread_rng();
        let id = (&mut rng)
            .sample_iter(Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        SocketBuilder {
            id,
            conn,
            rng,
            vsn: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        }
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

    // fn sign_root_session(&self, router, view, session, live_session) {

    // }
}

// struct Session {
//     id: socket.id,
//     view: view,
//     root_view: view,
//     router: router,
//     live_session: live_session_pair,
//     parent_pid: Option<>,
//     root_pid: Option<>,
//     session: session
// }

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
    csrf_token: String,
    #[serde(rename = "_mounts")]
    mounts: u32,
    #[serde(rename = "_track_static", default)]
    track_static: Vec<String>,
}

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
    WebsocketError(#[from] tungstenite::Error),
    #[error(transparent)]
    DeserializeError(#[from] serde_json::Error),
}
