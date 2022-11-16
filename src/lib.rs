//! Submillisecond LiveView provides rich, real-time user experiences with
//! server-rendered HTML.
//!
//! ### Prerequisites
//!
//! [Lunatic runtime] is required, along with the wasm32-wasi target.
//!
//! See [README.md#prerequisites] on how to install Lunatic.
//!
//! [Lunatic runtime]: https://github.com/lunatic-solutions/lunatic-rs#setup
//! [README.md#prerequisites]: https://github.com/lunatic-solutions/submillisecond-live-view#prerequisites
//!
//! ### Quick Start
//!
//! To get started, add `submillisecond`, `submillisecond-live-view`, and
//! `serde` to your Cargo.toml.
//!
//! ```text
//! [dependencies]
//! submillisecond = "*"
//! submillisecond-live-view = "*"
//! serde = { version = "*", features = ["derive"] }
//! ```
//!
//! Next, implement [`LiveView`] on a new type, and define the
//! [`LiveView::Events`] tuple, [`LiveView::mount`] and [`LiveView::render`]
//! methods.
//!
//! ```
//! use submillisecond_live_view::prelude::*;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct Counter {
//!     count: u32,
//! }
//!
//! impl LiveView for Counter {
//!     type Events = (Increment, Decrement);
//!
//!     fn mount(_uri: Uri, _socket: Option<Socket>) -> Self {
//!         Counter { count: 0 }
//!     }
//!
//!     fn render(&self) -> Rendered {
//!         html! {
//!             p { "Count is " (self.count) }
//!             button @click=(Increment) { "Increment" }
//!             button @click=(Decrement) { "Decrement" }
//!         }
//!     }
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct Increment {}
//!
//! impl LiveViewEvent<Increment> for Counter {
//!     fn handle(state: &mut Self, _event: Increment) {
//!         state.count += 1;
//!     }
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct Decrement {}
//!
//! impl LiveViewEvent<Decrement> for Counter {
//!     fn handle(state: &mut Self, _event: Decrement) {
//!         state.count -= 1;
//!     }
//! }
//! ```
//!
//! Finally, serve your submillisecond app with the `Counter`.
//!
//! ```
//! use submillisecond::{router, Application};
//!
//! fn main() -> std::io::Result<()> {
//!     Application::new(router! {
//!         GET "/" => Counter::handler()
//!     })
//!     .serve("127.0.0.1:3000")
//! }
//! ```
//!
//! ### Html Macro
//!
//! The `html!` macro is an extended version of the [maud] macro,
//! which is available under [`submillisecond_live_view::html!`](html!).
//!
//! Docs for the syntax of the `html!` macro are available on the maud website,
//! but this section documents some syntax features which are specific to
//! Submillisecond LiveView.
//!
//! [maud]: https://maud.lambda.xyz/
//!
//! #### Events
//!
//! Events can be defined with the `@click=(Increment)` syntax.
//! Where `click` is the event name, and `Increment` is the event to be sent
//! back to the server.
//!
//! This is syntax sugar for `phx-click=(std::any::type_name::<Increment>())`.
//!
//! **Example**
//!
//! ```rust
//! html! {
//!   button @click=(Greet) { "Greet" }
//! }
//! ```
//!
//! See <https://hexdocs.pm/phoenix_live_view/bindings.html#click-events>.
//!
//! #### Values
//!
//! Values can be added to events with the `:name=(value)` syntax.
//! Where `name` is the name of the variable, and `value` is the value.
//! It is typically used along side events to pass data back with the event.
//!
//! This is syntax sugar for `phx-value-name=(value)`.
//!
//! **Example**
//!
//! ```rust
//! html! {
//!   button :username=(user.name) @click=(Register) { "Register" }
//! }
//! ```
//!
//! See <https://hexdocs.pm/phoenix_live_view/bindings.html#click-events>.
//!
//! #### Nesting Html
//!
//! Maud supports [partials], but there is a different syntax for nesting
//! renders when using Submillisecond LiveView.
//!
//! Nested renders should use the `@(nested)` syntax.
//! If HTML created with the `html!` macro is nested without the `@` prefix,
//! then it will be rendered as a static string on the page and the content will
//! not be dynamic.
//!
//! **Example**
//!
//! ```rust
//! fn render_header(&self) -> Rendered {
//!   html! {
//!     h1 { "Header" }
//!   }
//! }
//!
//! fn render(&self) -> Rendered {
//!   html! {
//!     @(self.render_header())
//!   }
//! }
//! ```
//!
//! [partials]: https://maud.lambda.xyz/partials.html

#![warn(missing_docs)]

pub mod handler;
pub mod head;
pub mod rendered;
pub mod socket;

mod csrf;
mod event_handler;
mod live_view;
mod manager;
mod maud;

#[doc(hidden)]
pub use maud_live_view;
pub use maud_live_view::html;

pub use crate::live_view::*;

/// Prelude
pub mod prelude {
    pub use submillisecond::http::Uri;

    pub use crate::handler::LiveViewRouter;
    pub use crate::head::*;
    pub use crate::rendered::Rendered;
    pub use crate::socket::Socket;
    pub use crate::*;
}
