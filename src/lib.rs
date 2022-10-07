pub mod csrf;
pub mod socket;
pub mod tera;

use serde::Deserialize;
use socket::{Event, Socket};
use submillisecond::response::Response;
use submillisecond::RequestContext;

#[macro_export]
macro_rules! live_view {
    ($t: path, $path: expr) => {
        (|req: submillisecond::RequestContext| -> submillisecond::response::Response {
            match lunatic::process::ProcessRef::<$crate::tera::LiveViewTera<$t>>::lookup(
                stringify!($t $path),
            ) {
                Some(live_view) => $crate::LiveViewHandler::handle(&live_view, req),
                None => {
                    panic!("live view process not found");
                }
            }
        }) as submillisecond::Router
    };
}

pub trait LiveViewHandler {
    fn handle(&self, req: RequestContext) -> Response;
}

pub trait LiveView: Sized {
    type Events: EventList<Self>;

    fn mount(socket: Option<&Socket>) -> Self;

    fn not_found(_req: RequestContext) -> Response {
        submillisecond::defaults::err_404()
    }
}

pub trait LiveViewEvent<E> {
    const NAME: &'static str;

    fn handle(state: &mut Self, event: E, event_type: String);
}

pub trait EventList<T> {
    fn handle_event(state: &mut T, event: Event) -> Result<bool, serde_json::Error>;
}

impl<T> EventList<T> for () {
    fn handle_event(_state: &mut T, _event: Event) -> Result<bool, serde_json::Error> {
        Ok(false)
    }
}

impl<T, A> EventList<T> for (A,)
where
    T: LiveViewEvent<A>,
    A: for<'de> Deserialize<'de>,
{
    fn handle_event(state: &mut T, event: Event) -> Result<bool, serde_json::Error> {
        if <T as LiveViewEvent<A>>::NAME == event.name {
            let value: A = serde_json::from_value(event.value)?;
            T::handle(state, value, event.ty);
            return Ok(true);
        }

        Ok(false)
    }
}
