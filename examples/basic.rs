use serde::Serialize;
use submillisecond::{
    response::Response,
    router, static_router,
    websocket::{WebSocket, WebSocketUpgrade},
    Application, Handler, RequestContext,
};
use subview::{live_view, socket::Socket, Assigns, LiveView, LiveViewRender};

fn main() -> std::io::Result<()> {
    Application::new(router! {
        "/static" => static_router!("./static")
        // GET "/liveview" => liveview
        "/" => live_view!(Chat, "templates/foo.html")
    })
    .serve("127.0.0.1:3000")
}

// fn live_view(req: RequestContext) -> Response {
//     lunatic::process_local! {
//         pub static LIVE_VIEW: std::cell::RefCell<LiveView> = std::cell::RefCell::new(
//             LiveView::new("templates/**/*").expect("live view templates failed to compile"),
//         );
//     }

//     LIVE_VIEW.with_borrow(|live_view| submillisecond::Handler::handle(&live_view, req))
// }

// fn liveview(ws: WebSocket) -> WebSocketUpgrade {
//     ws.on_upgrade(|conn| {
//         let mut socket = Socket::new(conn);
//         socket.receive();
//     })
// }

// fn foo() {}

#[derive(Default, Serialize)]
struct Chat {
    name: String,
    age: i32,
}

impl LiveView for Chat {
    // fn render(&self, assigns: &Assigns) -> LiveViewRender {
    //     lunatic::process_local! {
    //         pub static LIVE_VIEW: std::cell::RefCell<LiveViewApp> = std::cell::RefCell::new(
    //             LiveViewApp::new("templates/**/*").expect("live view templates failed to compile"),
    //         );
    //     }

    //     LIVE_VIEW.with(|live_view| LiveViewRender::new(vec![], vec![]))
    // }

    fn mount(socket: &Socket) -> Self {
        Self::default()
    }
}
