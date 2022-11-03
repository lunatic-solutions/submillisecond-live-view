use std::fmt;

use serde::Serialize;
use submillisecond::response::Response;
use submillisecond::RequestContext;

use crate::socket::{Event, JoinEvent};

/// Handles requests and events.
pub trait LiveViewManager<T> {
    type State;
    type Reply: Serialize;
    type Error: fmt::Display;

    /// Handle an initial stateless request.
    fn handle_request(&self, req: RequestContext) -> Response;

    /// Handle a join event returning state and a reply.
    fn handle_join(
        &self,
        event: JoinEvent,
    ) -> LiveViewManagerResult<Join<T, Self::State, Self::Reply>, Self::Error>;

    /// Handle an event.
    fn handle_event(
        &self,
        event: Event,
        state: &mut Self::State,
        live_view: &T,
    ) -> LiveViewManagerResult<Self::Reply, Self::Error>;
}

/// Live view socket result for returning a response with a recoverable error,
/// or fatal error.
///
/// If fatal error is returned, the websocket connection is closed.
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
