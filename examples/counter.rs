use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::prelude::*;

fn main() -> std::io::Result<()> {
    Application::new(router! {
        GET "/" => Counter::handler()
        "/static" => static_router!("./static")
    })
    .serve("127.0.0.1:3000")
}

#[derive(Clone, Serialize, Deserialize)]
struct Counter {
    count: i32,
}

impl LiveView for Counter {
    type Events = ();

    fn mount(_uri: Uri, _socket: Option<Socket>) -> Self {
        Counter { count: 0 }
    }

    fn render(&self) -> Rendered<Self> {
        html! {
            button @click=(|state| state.count += 1) {
                "Increment"
            }
            button @click=(|state| state.count -= 1) {
                "Decrement"
            }
            p { "Count is " (self.count) }
            @if self.count >= 5 {
                p { "Count is high!" }
            }
        }
    }

    fn head() -> Head {
        Head::defaults()
            .with_title("LiveView Counter")
            .with_style(Style::Link("/static/counter.css"))
    }
}
