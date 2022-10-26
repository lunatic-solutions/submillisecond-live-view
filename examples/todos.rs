fn main() {}

// use serde::{Deserialize, Serialize};
// use submillisecond::{router, static_router, Application};
// use submillisecond_live_view::socket::Socket;
// use submillisecond_live_view::tera::{LiveViewContext, LiveViewTera};
// use submillisecond_live_view::{CheckboxValue, LiveView, LiveViewEvent};
// use uuid::Uuid;

// fn main() -> std::io::Result<()> {
//     LiveViewContext::init(b"some-secret-key", "templates/layout.html");

//     Application::new(router! {
//         "/" => LiveViewTera::<Todos>::route("templates/todos.html")
//         "/static" => static_router!("./static")
//     })
//     .serve("127.0.0.1:3000")
// }

// #[derive(Clone, Serialize, Deserialize)]
// struct Todos {
//     filter: String,
//     todos: Vec<Todo>,
// }

// #[derive(Clone, Serialize, Deserialize)]
// struct Todo {
//     id: Uuid,
//     title: String,
//     completed: bool,
//     editing: bool,
// }

// impl Todo {
//     fn new(title: String) -> Self {
//         Todo {
//             id: Uuid::new_v4(),
//             title,
//             completed: false,
//             editing: false,
//         }
//     }
// }

// impl LiveView for Todos {
//     type Events = (
//         Add,
//         Remove,
//         Toggle,
//         Edit,
//         ToggleEdit,
//         ClearCompleted,
//         SetFilter,
//     );

//     fn mount(_socket: Option<&Socket>) -> Self {
//         Todos {
//             filter: "all".to_string(),
//             todos: vec![],
//         }
//     }
// }

// #[derive(Deserialize)]
// struct Add {
//     title: String,
// }

// impl LiveViewEvent<Add> for Todos {
//     const NAME: &'static str = "add_todo";

//     fn handle(state: &mut Self, event: Add, _event_type: String) {
//         state.todos.push(Todo::new(event.title));
//     }
// }

// #[derive(Deserialize)]
// struct Remove {
//     id: Uuid,
// }

// impl LiveViewEvent<Remove> for Todos {
//     const NAME: &'static str = "remove_todo";

//     fn handle(state: &mut Self, event: Remove, _event_type: String) {
//         state.todos.retain(|todo| todo.id != event.id);
//     }
// }

// #[derive(Deserialize)]
// struct Toggle {
//     id: Uuid,
//     value: CheckboxValue,
// }

// impl LiveViewEvent<Toggle> for Todos {
//     const NAME: &'static str = "toggle_todo";

//     fn handle(state: &mut Self, event: Toggle, _event_type: String) {
//         if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id ==
// event.id) {             todo.completed = event.value.is_checked();
//         }
//     }
// }

// #[derive(Deserialize)]
// struct Edit {
//     id: Uuid,
//     title: String,
// }

// impl LiveViewEvent<Edit> for Todos {
//     const NAME: &'static str = "edit_todo";

//     fn handle(state: &mut Self, event: Edit, _event_type: String) {
//         if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id ==
// event.id) {             todo.title = event.title;
//             todo.editing = false;
//         }
//     }
// }

// #[derive(Deserialize)]
// struct ToggleEdit {
//     id: Uuid,
//     detail: u8,
// }

// impl LiveViewEvent<ToggleEdit> for Todos {
//     const NAME: &'static str = "toggle_edit_todo";

//     fn handle(state: &mut Self, event: ToggleEdit, _event_type: String) {
//         if event.detail == 2 {
//             if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id ==
// event.id) {                 todo.editing = true;
//             }
//         }
//     }
// }

// #[derive(Deserialize)]
// struct ClearCompleted {}

// impl LiveViewEvent<ClearCompleted> for Todos {
//     const NAME: &'static str = "clear_completed_todos";

//     fn handle(state: &mut Self, _event: ClearCompleted, _event_type: String)
// {         state.todos.retain(|todo| !todo.completed);
//     }
// }

// #[derive(Deserialize)]
// struct SetFilter {
//     filter: String,
// }

// impl LiveViewEvent<SetFilter> for Todos {
//     const NAME: &'static str = "set_filter";

//     fn handle(state: &mut Self, event: SetFilter, _event_type: String) {
//         state.filter = event.filter;
//     }
// }
