use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use subview::socket::Socket;
use subview::tera::LiveViewTera;
use subview::{LiveView, LiveViewEvent};

fn main() -> std::io::Result<()> {
    Application::new(router! {
        "/" => LiveViewTera::<Chat>::route("templates/layout.html", "templates/index.html") // LiveViewRoute::<LiveViewTera<Chat>, Chat>::new("templates/layout.html", "templates/index.html")
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
    type Events = (Increment,);

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
        state.name = "Jim".to_string();
        state.age = 32;
        println!("Count = {}", state.count);
    }
}
