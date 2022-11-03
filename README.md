# submillisecond-live-view

A live-view implementation for the [submillisecond] web framework built with [lunatic].

# Features

- todo!()

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

## Getting started with submillisecond-live-view

Add it as a dependency

```toml
submillisecond-live-view = "*"
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

Build JS

```bash
cd web
npm i
npm run build
cd ..
```

Finally, run an example

```bash
cargo run --example counter
```

# License

Licensed under either of

- Apache License, Version 2.0, (http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (http://opensource.org/licenses/MIT)

at your option.

[lunatic]: https://lunatic.solutions
[submillisecond]: https://github.com/lunatic-solutions/submillisecond
