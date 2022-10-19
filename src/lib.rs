pub mod csrf;
pub mod handler;
pub mod manager;
pub mod socket;
pub mod tera;

use serde::Deserialize;
use socket::{Event, Socket};
use submillisecond::response::Response;
use submillisecond::RequestContext;

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
