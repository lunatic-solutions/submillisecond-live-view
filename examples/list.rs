use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::prelude::*;

fn main() -> std::io::Result<()> {
    Application::new(router! {
        GET "/" => List::handler()
        "/static" => static_router!("./static")
    })
    .serve("127.0.0.1:3000")
}

#[derive(Clone, Serialize, Deserialize)]
struct List {
    numbers: Vec<usize>,
}

impl LiveView for List {
    type Events = (Add, Remove, IncrementLast);

    fn mount(_uri: Uri, _socket: Option<Socket<Self>>) -> Self {
        List {
            numbers: Vec::new(),
        }
    }

    fn render(&self) -> Rendered {
        html! {
            button @click=(Add) { "Increment" }
            button @click=(Remove) { "Decrement" }
            button @click=(IncrementLast) { "Increment last" }
            ul {
                @for num in &self.numbers {
                    li { (num) }
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct Add {}

impl LiveViewEvent<Add> for List {
    fn handle(state: &mut Self, _event: Add) {
        state
            .numbers
            .push(state.numbers.last().map(|last| last + 1).unwrap_or(0));
    }
}

#[derive(Deserialize)]
struct Remove {}

impl LiveViewEvent<Remove> for List {
    fn handle(state: &mut Self, _event: Remove) {
        state.numbers.pop();
    }
}

#[derive(Deserialize)]
struct IncrementLast {}

impl LiveViewEvent<IncrementLast> for List {
    fn handle(state: &mut Self, _event: IncrementLast) {
        if let Some(last) = state.numbers.last_mut() {
            *last += 1;
        }
    }
}
