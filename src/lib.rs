pub mod csrf;
pub mod socket;
pub mod tera;

use std::{
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
    path::Path,
};

use lunatic_log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use socket::{Event, Socket, SocketError, SocketMessage};
use submillisecond::{
    extract::FromOwnedRequest,
    http::{header::UPGRADE, StatusCode},
    response::{IntoResponse, Response},
    websocket::WebSocket,
    Handler, RequestContext,
};

#[macro_export]
macro_rules! live_view {
    ($t: path, $path: expr) => {
        // (|mut req: submillisecond::RequestContext| -> submillisecond::response::Response {
        //     lunatic::process_local! {
        //         pub static LIVE_VIEW: std::cell::RefCell<$crate::tera::LiveViewTera<$t>> = std::cell::RefCell::new(
        //             $crate::tera::LiveViewTera::new($path, submillisecond::defaults::err_404).expect("live view templates failed to compile"),
        //         );
        //     }

        //     LIVE_VIEW.with_borrow(|live_view| ::submillisecond::Handler::handle(&*live_view, req))
        // }) as submillisecond::Router

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

// pub trait LiveViewHandler {
//     type Handler: Handler;

//     fn live_view_handler(template: &str) -> Self::Handler;
// }

pub trait LiveViewHandler {
    fn handle(&self, req: RequestContext) -> Response;
}

pub trait LiveView: Sized {
    type Events: EventList<Self>;

    fn mount(socket: Option<&Socket>) -> Self;

    fn not_found(req: RequestContext) -> Response {
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

impl<T, A> EventList<T> for A
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

pub struct LiveViewRender {
    statics: Vec<String>,
    dynamics: Vec<serde_json::Value>,
}

impl LiveViewRender {
    pub fn new(statics: Vec<String>, dynamics: Vec<serde_json::Value>) -> Self {
        debug_assert_eq!(
            statics.len(),
            dynamics.len() + 1,
            "static items should be 1 longer than dynamic items"
        );

        LiveViewRender { statics, dynamics }
    }
}

#[derive(Default)]
pub struct Assigns {
    assigns: BTreeMap<String, Cd<Value>>,
}

impl Assigns {
    pub fn new() -> Self {
        Assigns::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, val: impl Into<Value>) {
        self.assigns.insert(key.into(), Cd::new_true(val.into()));
    }

    pub fn reset_all(&mut self) {
        for val in self.assigns.values_mut() {
            val.reset();
        }
    }
}

// pub enum Value {
//     Null,
//     Bool(bool),
//     Number(Number),
//     String(String),
//     // TODO: `Cd<Value>` change detection per item in array
//     Array(Vec<Value>),
//     // TODO: `Cd<Value>` change detection per item in map
//     Object(Map<String, Value>),
// }
