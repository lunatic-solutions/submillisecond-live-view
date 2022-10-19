use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use subview::socket::Socket;
use subview::tera::{LiveViewContext, LiveViewTera};
use subview::{LiveView, LiveViewEvent};

fn main() -> std::io::Result<()> {
    LiveViewContext::init(b"some-secret-key", "templates/layout.html");

    Application::new(router! {
        "/" => LiveViewTera::<Counter>::route("templates/index.html")
        "/static" => static_router!("./static")
    })
    .serve("127.0.0.1:3000")
}

#[derive(Clone, Serialize, Deserialize)]
struct Counter {
    count: i32,
}

impl LiveView for Counter {
    type Events = (Increment, Decrement);

    fn mount(_socket: Option<&Socket>) -> Self {
        Counter { count: 0 }
    }
}

#[derive(Deserialize)]
struct Increment {}

impl LiveViewEvent<Increment> for Counter {
    const NAME: &'static str = "increment";

    fn handle(state: &mut Self, _event: Increment, _event_type: String) {
        state.count += 1;
    }
}

#[derive(Deserialize)]
struct Decrement {}

impl LiveViewEvent<Decrement> for Counter {
    const NAME: &'static str = "decrement";

    fn handle(state: &mut Self, _event: Decrement, _event_type: String) {
        state.count -= 1;
    }
}
