pub mod change_detection;
pub mod csrf;
pub mod socket;
pub mod tera;

use std::{
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
    path::Path,
};

use change_detection::Cd;
use lunatic_log::{info, warn};
use serde::Serialize;
use serde_json::{Map, Number, Value};
use socket::{Socket, SocketError, SocketMessage};
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
        (|mut req: submillisecond::RequestContext| -> submillisecond::response::Response {
            lunatic::process_local! {
                pub static LIVE_VIEW: std::cell::RefCell<$crate::tera::LiveViewTera<$t>> = std::cell::RefCell::new(
                    $crate::tera::LiveViewTera::new($path, submillisecond::defaults::err_404).expect("live view templates failed to compile"),
                );
            }

            LIVE_VIEW.with_borrow(|live_view| ::submillisecond::Handler::handle(&*live_view, req))
        }) as submillisecond::Router
    };
}

pub trait LiveViewHandler<Arg, Ret> {
    type Handler: Handler<Arg, Ret>;

    fn live_view_handler(template: &str) -> Self::Handler;
}

pub trait LiveView {
    fn mount(socket: &Socket) -> Self;
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
