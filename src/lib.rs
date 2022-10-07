pub mod csrf;
pub mod socket;
pub mod tera;

use std::marker::PhantomData;

use lunatic::process::StartProcess;
use serde::{Deserialize, Serialize};
use socket::{Event, Socket};
use submillisecond::response::Response;
use submillisecond::{Handler, RequestContext};

pub struct LiveViewRoute<T> {
    layout: &'static str,
    template: &'static str,
    phantom: PhantomData<T>,
}

impl<T> LiveViewRoute<T> {
    pub const fn new(layout: &'static str, template: &'static str) -> LiveViewRoute<T> {
        LiveViewRoute {
            layout,
            template,
            phantom: PhantomData,
        }
    }
}

impl<T> Handler for LiveViewRoute<T>
where
    T: Clone + LiveView + Serialize + for<'de> Deserialize<'de>,
{
    fn handle(&self, mut req: RequestContext) -> Response {
        let cursor = req.reader.cursor;
        let process_id = format!(
            "{}-{}-{}",
            std::any::type_name::<T>(),
            self.layout,
            self.template,
        );
        req.reader.cursor = cursor;

        match lunatic::process::ProcessRef::<tera::LiveViewTera<T>>::lookup(&process_id) {
            Some(live_view) => crate::LiveViewHandler::handle(&live_view, req),
            None => {
                let process_ref = tera::LiveViewTera::<T>::start(
                    (self.layout.into(), self.template.into()),
                    Some(&process_id),
                );
                process_ref.handle(req)
            }
        }
    }
}

pub trait LiveViewHandler {
    fn handle(&self, req: RequestContext) -> Response;
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
