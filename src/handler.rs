use std::marker::PhantomData;

use lunatic_log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use submillisecond::extract::FromOwnedRequest;
use submillisecond::http::header;
use submillisecond::response::{IntoResponse, Response};
use submillisecond::websocket::WebSocket;
use submillisecond::{Handler, RequestContext};

use crate::manager::{Join, LiveViewManager, LiveViewManagerResult};
use crate::maud::LiveViewMaud;
use crate::socket::{Message, ProtocolEvent, Socket, SocketError, SocketMessage};
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
    L: LiveViewManager<T> + Clone + Serialize + for<'de> Deserialize<'de>,
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
                let mut state = None;
                let mut socket = Socket::new(conn);

                loop {
                    match socket.receive() {
                        Ok(SocketMessage::Event(message)) => {
                            if !handle_message(&mut socket, &live_view, message, &mut state) {
                                break;
                            }
                        }
                        Ok(SocketMessage::Ping(_)) | Ok(SocketMessage::Pong(_)) => {}
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

fn handle_message<L, T>(
    socket: &mut Socket,
    manager: &L,
    mut message: Message,
    state: &mut Option<(T, L::State)>,
) -> bool
where
    L: LiveViewManager<T>,
    T: LiveView,
{
    info!("Received message: {message:?}");
    match message.event {
        ProtocolEvent::Close => {
            info!("Client left");
            false
        }
        ProtocolEvent::Error => true,
        ProtocolEvent::Event => match message.take_event() {
            Ok(event) => match state.as_mut() {
                Some((live_view, state)) => {
                    match <T::Events as EventList<T>>::handle_event(live_view, event.clone()) {
                        Ok(handled) => {
                            if !handled {
                                warn!("received unknown event");
                                return true;
                            }
                        }
                        Err(err) => {
                            warn!("failed to deserialize event: {err}");
                            return true;
                        }
                    }

                    let result = manager.handle_event(event, state, live_view);
                    match result {
                        LiveViewManagerResult::Ok(reply) => {
                            match socket.send(message.reply_ok(reply)) {
                                Ok(_) => true,
                                Err(SocketError::WebsocketError(
                                    tungstenite::Error::AlreadyClosed
                                    | tungstenite::Error::ConnectionClosed,
                                )) => {
                                    warn!("failed to reply, connection closed");
                                    false
                                }
                                Err(err) => {
                                    error!("{err}");
                                    true
                                }
                            }
                        }
                        LiveViewManagerResult::Error(err) => {
                            error!("{err}");
                            true
                        }
                        LiveViewManagerResult::FatalError(err) => {
                            error!("fatal: {err}");
                            false
                        }
                    }
                }
                None => {
                    warn!("event received before mount");
                    true
                }
            },
            Err(err) => {
                error!("{err}");
                true
            }
        },
        ProtocolEvent::Heartbeat => {
            if let Err(err) = socket.send(message.reply_ok(Map::default())) {
                error!("{err}");
            }
            true
        }
        ProtocolEvent::Join => {
            let join_event = message.take_join_event().expect("invalid join event");
            match manager.handle_join(join_event) {
                LiveViewManagerResult::Ok(Join {
                    live_view,
                    state: new_state,
                    reply,
                }) => {
                    *state = Some((live_view, new_state));
                    socket.send(message.reply_ok(reply)).unwrap();
                    true
                }
                LiveViewManagerResult::Error(err) => {
                    error!("{err}");
                    true
                }
                LiveViewManagerResult::FatalError(err) => {
                    error!("fatal: {err}");
                    false
                }
            }
        }
        ProtocolEvent::Leave => {
            info!("Client left");
            false
        }
        ProtocolEvent::Reply => true,
    }
}
