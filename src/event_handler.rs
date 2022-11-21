use lunatic::serializer::Json;
use lunatic::{Mailbox, Process, Tag};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::manager::{Join, LiveViewManager};
use crate::socket::{Event, JoinEvent, RawSocket, Socket};
use crate::{EventList, LiveView};

pub type AnonymousEventHandler<T> = fn(state: &mut T, event_name: &str) -> bool;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EventHandler {
    event_handler: Process<EventHandlerMessage, Json>,
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

impl EventHandler {
    pub(crate) fn spawn<L, T>(socket: RawSocket, manager: L) -> Self
    where
        L: LiveViewManager<T> + Serialize + for<'de> Deserialize<'de>,
        T: LiveView,
    {
        let process = Process::spawn_link((socket, manager), event_handler);
        EventHandler {
            event_handler: process,
        }
    }

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
                                event_handler: this,
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
                        println!("Handling anonymous event");
                        let handled = if !state.handle_anonymous_event(live_view, &event.name) {
                            <T::Events as EventList<T>>::handle_event(live_view, event.clone())
                        } else {
                            Ok(true)
                        };

                        println!("we here now");

                        match handled {
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
