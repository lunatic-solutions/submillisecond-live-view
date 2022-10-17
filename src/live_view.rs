use std::fmt;

use serde::Serialize;
use submillisecond::response::Response;
use submillisecond::RequestContext;

use crate::socket::{Event, JoinEvent};

pub trait LiveViewSocket<T> {
    type State;
    type Reply: Serialize;
    type Error: fmt::Display;

    fn handle_request(&self, req: RequestContext) -> Response;
    fn handle_join(
        &self,
        event: JoinEvent,
        values: &T,
    ) -> LiveViewSocketResult<(Self::State, Self::Reply), Self::Error>;
    fn handle_event(
        &self,
        state: &mut Self::State,
        event: Event,
        values: &T,
    ) -> LiveViewSocketResult<Self::Reply, Self::Error>;
}

pub enum LiveViewSocketResult<T, E> {
    Ok(T),
    Error(E),
    FatalError(E),
}
