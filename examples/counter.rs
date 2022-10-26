use maud::html;
use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::maud::{LiveViewContext, LiveViewMaud};
use submillisecond_live_view::rendered::Rendered;
use submillisecond_live_view::{LiveView, LiveViewEvent};

fn main() -> std::io::Result<()> {
    LiveViewContext::init(b"some-secret-key");

    Application::new(router! {
        "/" => LiveViewMaud::<Counter>::route()
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

    fn render(&self) -> Rendered {
        html! {
            button phx-click="increment" {
                "Increment"
            }
            button phx-click="decrement" {
                "Decrement"
            }
            p {
                "Count is " (self.count)
            }
            @if self.count >= 5 {
                p { "Count is high!" }
            }
        }
    }

    fn mount() -> Self {
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
