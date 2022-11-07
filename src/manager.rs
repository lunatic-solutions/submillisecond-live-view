use std::fmt;

use serde::{Deserialize, Serialize};
use submillisecond::response::Response;
use submillisecond::RequestContext;

use crate::socket::{Event, JoinEvent, Socket};
use crate::LiveView;

/// Handles requests and events.
pub trait LiveViewManager<T>
where
    Self: Sized,
    T: LiveView,
{
    type State: Serialize + for<'de> Deserialize<'de>;
    type Reply: Serialize;
    type Error: fmt::Display;

    /// Handle an initial stateless request.
    fn handle_request(&self, req: RequestContext) -> Response;

    /// Handle a join event returning state and a reply.
    fn handle_join(
        &self,
        socket: Socket<Self, T>,
        event: JoinEvent,
    ) -> LiveViewManagerResult<Join<T, Self::State, Self::Reply>, Self::Error>;

    /// Handle an event.
    fn handle_event(
        &self,
        event: Event,
        state: &mut Self::State,
        live_view: &T,
    ) -> LiveViewManagerResult<Option<Self::Reply>, Self::Error>;
}

/// Live view socket result for returning a response with a recoverable error,
/// or fatal error.
///
/// If fatal error is returned, the websocket connection is closed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum LiveViewManagerResult<T, E> {
    Ok(T),
    Error(E),
    FatalError(E),
}

pub struct Join<L, S, R> {
    pub live_view: L,
    pub state: S,
    pub reply: R,
}

impl<T, E> LiveViewManagerResult<T, E> {
    pub fn into_result(self) -> Result<T, E> {
        match self {
            LiveViewManagerResult::Ok(value) => Ok(value),
            LiveViewManagerResult::Error(err) | LiveViewManagerResult::FatalError(err) => Err(err),
        }
    }
}
