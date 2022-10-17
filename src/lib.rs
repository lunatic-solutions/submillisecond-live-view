pub mod csrf;
mod live_view;
pub mod socket;
pub mod tera;

use std::env;
use std::marker::PhantomData;

use live_view::LiveViewSocket;
use lunatic_log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use socket::{Event, Message, Socket, SocketError, SocketMessage};
use submillisecond::extract::FromOwnedRequest;
use submillisecond::http::header;
use submillisecond::response::{IntoResponse, Response};
use submillisecond::websocket::WebSocket;
use submillisecond::{Handler, RequestContext};

use crate::live_view::LiveViewSocketResult;
use crate::socket::ProtocolEvent;

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
    L: LiveViewSocket<T> + Clone + Serialize + for<'de> Deserialize<'de>,
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
                let socket_builder = Socket::builder(conn);
                let mut socket = match env::var("SUBMILLISECOND_LIVE_VIEW_SECRET_KEY")
                    .ok()
                    .and_then(|key| key.into_bytes().try_into().ok())
                {
                    Some(secret_key) => socket_builder.secret_key(secret_key),
                    None => socket_builder.generate_secret_key(),
                };

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
    live_view: &L,
    mut message: Message,
    state: &mut Option<(T, L::State)>,
) -> bool
where
    L: LiveViewSocket<T>,
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
                Some((state, socket_state)) => {
                    match <T::Events as EventList<T>>::handle_event(state, event.clone()) {
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

                    let result = live_view.handle_event(socket_state, event, state);
                    match result {
                        LiveViewSocketResult::Ok(reply) => {
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
                        LiveViewSocketResult::Error(err) => {
                            error!("{err}");
                            true
                        }
                        LiveViewSocketResult::FatalError(err) => {
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
            let mount_state = T::mount(Some(socket));
            match live_view.handle_join(join_event, &mount_state) {
                LiveViewSocketResult::Ok((new_state, reply)) => {
                    *state = Some((mount_state, new_state));
                    socket.send(message.reply_ok(reply)).unwrap();
                    true
                }
                LiveViewSocketResult::Error(err) => {
                    error!("{err}");
                    true
                }
                LiveViewSocketResult::FatalError(err) => {
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

/// A live view.
pub trait LiveView: Sized {
    type Events: EventList<Self>;

    fn mount(socket: Option<&Socket>) -> Self;

    fn not_found(_req: RequestContext) -> Response {
        submillisecond::defaults::err_404()
    }
}

/// Live view event handler.
pub trait LiveViewEvent<E> {
    const NAME: &'static str;

    fn handle(state: &mut Self, event: E, event_type: String);
}

/// Event list is a trait to handle an incoming live view event.
pub trait EventList<T> {
    fn handle_event(state: &mut T, event: Event) -> Result<bool, serde_json::Error>;
}

impl<T> EventList<T> for () {
    fn handle_event(_state: &mut T, _event: Event) -> Result<bool, serde_json::Error> {
        Ok(false)
    }
}

macro_rules! impl_event_list {
    ($( $t: ident ),*) => {
        impl<T, $( $t ),*> EventList<T> for ($( $t, )*)
        where
            $(
                T: LiveViewEvent<$t>,
                $t: for<'de> Deserialize<'de>,
            )*
        {
            fn handle_event(state: &mut T, event: Event) -> Result<bool, serde_json::Error> {
                $(
                    if <T as LiveViewEvent<$t>>::NAME == event.name {
                        let value: $t = serde_json::from_value(event.value)?;
                        T::handle(state, value, event.ty);
                        return Ok(true);
                    }
                )*

                Ok(false)
            }
        }
    };
}

impl_event_list!(A);
impl_event_list!(A, B);
impl_event_list!(A, B, C);
impl_event_list!(A, B, C, D);
impl_event_list!(A, B, C, D, E);
impl_event_list!(A, B, C, D, E, F);
impl_event_list!(A, B, C, D, E, F, G);
impl_event_list!(A, B, C, D, E, F, G, H);
impl_event_list!(A, B, C, D, E, F, G, H, I);
impl_event_list!(A, B, C, D, E, F, G, H, I, J);
impl_event_list!(A, B, C, D, E, F, G, H, I, J, K);
impl_event_list!(A, B, C, D, E, F, G, H, I, J, K, L);
