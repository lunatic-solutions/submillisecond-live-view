use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::prelude::*;

fn main() -> std::io::Result<()> {
    Application::new(router! {
        GET "/" => Counter::handler("examples/counter.html", "#app")
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

    fn mount(_uri: Uri, _socket: Option<Socket>) -> Self {
        Counter { count: 0 }
    }

    fn render(&self) -> Rendered {
        html! {
            button @click=(Increment) { "Increment" }
            button @click=(Decrement) { "Decrement" }
            p { "Count is " (self.count) }
            @if self.count >= 5 {
                p { "Count is high!" }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Increment {}

impl LiveViewEvent<Increment> for Counter {
    fn handle(state: &mut Self, _event: Increment) {
        state.count += 1;
    }
}

#[derive(Serialize, Deserialize)]
struct Decrement {}

impl LiveViewEvent<Decrement> for Counter {
    fn handle(state: &mut Self, _event: Decrement) {
        state.count -= 1;
    }
}
