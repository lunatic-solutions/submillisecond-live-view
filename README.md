# submillisecond-live-view

A live-view implementation for the [submillisecond] web framework built with [lunatic].

# Features

- todo!()

# Code example

```rust
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
    type Events = (Increment,);

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

```

## Getting started with submillisecond-live-view

Add it as a dependency

```toml
submillisecond-live-view = "*"
```

# License

Licensed under either of

- Apache License, Version 2.0, (http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (http://opensource.org/licenses/MIT)

at your option.

[lunatic]: https://lunatic.solutions
[submillisecond]: https://github.com/lunatic-solutions/submillisecond
