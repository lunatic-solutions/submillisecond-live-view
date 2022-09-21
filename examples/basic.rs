use lunatic::process::StartProcess;
use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use subview::{live_view, socket::Socket, tera::LiveViewTera, LiveView, LiveViewEvent};

fn main() -> std::io::Result<()> {
    LiveViewTera::<Chat>::start_link(
        "templates/foo.html".into(),
        Some(stringify!(Chat "templates/foo.html")),
    );

    Application::new(router! {
        "/" => live_view!(Chat, "templates/foo.html")
        "/static" => static_router!("./static")
    })
    .serve("127.0.0.1:3000")
}

#[derive(Clone, Serialize, Deserialize)]
struct Chat {
    name: String,
    age: i32,
    count: i32,
}

impl LiveView for Chat {
    type Events = Increment;

    fn mount(_socket: Option<&Socket>) -> Self {
        Chat {
            name: "Ari".to_string(),
            age: 22,
            count: 0,
        }
    }
}

#[derive(Deserialize)]
struct Increment {}

impl LiveViewEvent<Increment> for Chat {
    const NAME: &'static str = "increment";

    fn handle(state: &mut Self, _event: Increment, _event_type: String) {
        state.count += 1;
        println!("Count = {}", state.count);
    }
}
