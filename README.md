# Submillisecond LiveView

A LiveView implementation for the [submillisecond] web framework built with [lunatic].

# What is LiveView?

LiveView provides rich, real-time user experiences with server-rendered HTML.

The LiveView programming model is declarative: instead of saying "once event X happens, change Y on the page",
events in LiveView are regular messages which may cause changes to its state. Once the state changes,
LiveView will re-render the relevant parts of its HTML template and push it to the browser,
which updates itself in the most efficient manner.
This means developers write LiveView templates as any other server-rendered HTML and LiveView does the hard work
of tracking changes and sending the relevant diffs to the browser.

It was made popular by the [Phoenix] webframework for Elixir.

[phoenix]: https://hexdocs.pm/phoenix_live_view/Phoenix.LiveView.html

# Prerequisites

[Lunatic runtime] is required, along with the wasm32-wasi target.

```bash
cargo install lunatic-runtime
rustup target add wasm32-wasi
```

It is also recommended to add a `.cargo/config.toml` file with the build target and runner configured.

```toml
# .cargo/config.toml

[build]
target = "wasm32-wasi"

[target.wasm32-wasi]
runner = "lunatic"
```

[lunatic runtime]: https://github.com/lunatic-solutions/lunatic-rs#setup

# Code example

```rust
use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::prelude::*;

fn main() -> std::io::Result<()> {
    Application::new(router! {
        "/" => Counter::handler()
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

    fn mount(_uri: Uri, _socket: Option<&mut Socket>) -> Self {
        Counter { count: 0 }
    }

    fn render(&self) -> Rendered {
        html! {
            button @click=(Increment) { "Increment" }
            button @click=(Decrement) { "Decrement" }
            p { "Count is " (self.count) }
        }
    }
}

#[derive(Deserialize)]
struct Increment {}

impl LiveViewEvent<Increment> for Counter {
    fn handle(state: &mut Self, _event: Increment) {
        state.count += 1;
    }
}

#[derive(Deserialize)]
struct Decrement {}

impl LiveViewEvent<Decrement> for Counter {
    fn handle(state: &mut Self, _event: Decrement) {
        state.count -= 1;
    }
}
```

## Running examples

Clone the repository

```bash
git clone git@github.com:lunatic-solutions/submillisecond-live-view.git
cd submillisecond-live-view
```

Initialize submodules

```bash
git submodule init
```

Run an example

```bash
cargo run --example clock
```

# License

Licensed under either of

- Apache License, Version 2.0, (http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (http://opensource.org/licenses/MIT)

at your option.

[lunatic]: https://lunatic.solutions
[submillisecond]: https://github.com/lunatic-solutions/submillisecond
