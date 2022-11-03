use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::prelude::*;

fn main() -> std::io::Result<()> {
    Application::new(router! {
        "/" => List::handler()
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

    fn mount(_uri: Uri) -> Self {
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

    fn styles() -> &'static [&'static str] {
        &["/static/List.css"]
    }
}

#[derive(Deserialize)]
struct Add {}

impl LiveViewEvent<Add> for List {
    fn handle(state: &mut Self, _event: Add, _event_type: String) {
        state
            .numbers
            .push(state.numbers.last().map(|last| last + 1).unwrap_or(0));
    }
}

#[derive(Deserialize)]
struct Remove {}

impl LiveViewEvent<Remove> for List {
    fn handle(state: &mut Self, _event: Remove, _event_type: String) {
        state.numbers.pop();
    }
}

#[derive(Deserialize)]
struct IncrementLast {}

impl LiveViewEvent<IncrementLast> for List {
    fn handle(state: &mut Self, _event: IncrementLast, _event_type: String) {
        if let Some(last) = state.numbers.last_mut() {
            *last += 1;
        }
    }
}
