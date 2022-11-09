use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use submillisecond::response::Response;
use submillisecond::RequestContext;

use crate::socket::{Event, JoinEvent, Socket};
use crate::LiveView;

/// Handles requests and events.
pub(crate) trait LiveViewManager<T>
where
    Self: Sized,
    T: LiveView,
{
    type State: Serialize + for<'de> Deserialize<'de>;
    // type Reply: Serialize;
    type Error: fmt::Display;

    /// Handle an initial stateless request.
    fn handle_request(&self, req: RequestContext) -> Response;

    /// Handle a join event returning state and a reply.
    fn handle_join(
        &self,
        socket: Socket,
        event: JoinEvent,
    ) -> LiveViewManagerResult<Join<T, Self::State, Value>, Self::Error>;

    /// Handle an event.
    fn handle_event(
        &self,
        event: Event,
        state: &mut Self::State,
        live_view: &T,
    ) -> LiveViewManagerResult<Option<Value>, Self::Error>;
}

/// Live view socket result for returning a response with a recoverable error,
/// or fatal error.
///
/// If fatal error is returned, the websocket connection is closed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum LiveViewManagerResult<T, E> {
    Ok(T),
    Error(E),
    FatalError(E),
}

pub(crate) struct Join<L, S, R> {
    pub(crate) live_view: L,
    pub(crate) state: S,
    pub(crate) reply: R,
}

impl<T, E> LiveViewManagerResult<T, E> {
    pub(crate) fn into_result(self) -> Result<T, E> {
        match self {
            LiveViewManagerResult::Ok(value) => Ok(value),
            LiveViewManagerResult::Error(err) | LiveViewManagerResult::FatalError(err) => Err(err),
        }
    }
}
