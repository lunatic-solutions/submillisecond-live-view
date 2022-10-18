use std::fmt;

use serde::Serialize;
use submillisecond::response::Response;
use submillisecond::RequestContext;

use crate::socket::{Event, JoinEvent};

/// Handles requests and events.
pub trait LiveViewSocket<T> {
    type State;
    type Reply: Serialize;
    type Error: fmt::Display;

    /// Handle an initial stateless request.
    fn handle_request(&self, req: RequestContext) -> Response;

    /// Handle a join event returning state and a reply.
    fn handle_join(
        &self,
        event: JoinEvent,
        values: &T,
    ) -> LiveViewSocketResult<(Self::State, Self::Reply), Self::Error>;

    /// Handle an event.
    fn handle_event(
        &self,
        state: &mut Self::State,
        event: Event,
        values: &T,
    ) -> LiveViewSocketResult<Self::Reply, Self::Error>;
}

/// Live view socket result for returning a response with a recoverable error,
/// or fatal error.
///
/// If fatal error is returned, the websocket connection is closed.
pub enum LiveViewSocketResult<T, E> {
    Ok(T),
    Error(E),
    FatalError(E),
}
