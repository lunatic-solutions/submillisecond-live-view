use maud::html;
use serde::{Deserialize, Serialize};
use submillisecond::http::Uri;
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::maud::{LiveViewContext, LiveViewMaud};
use submillisecond_live_view::rendered::Rendered;
use submillisecond_live_view::{CheckboxValue, LiveView, LiveViewEvent};
use uuid::Uuid;

fn main() -> std::io::Result<()> {
    LiveViewContext::init(b"some-secret-key");

    Application::new(router! {
        "/" => LiveViewMaud::<Todos>::route()
        "/static" => static_router!("./static")
    })
    .serve("127.0.0.1:3000")
}

#[derive(Clone, Serialize, Deserialize)]
struct Todos {
    filter: Filter,
    todos: Vec<Todo>,
}

impl Todos {
    fn render_header(&self) -> Rendered {
        html! {
            header.header {
                h1 { "todos" }

                form #newtodo
                    method="post"
                    autocapitalize="off"
                    autocomplete="off"
                    autocorrect="off"
                    phx-submit="add_todo"
                    spellcheck="false"
                    url="#"
                {
                    i {
                        input #newtodo_text .new-todo autofocus name="title" placeholder="What needs to be done?" type="text";
                    }
                    button.hidden type="submit" { "submit" }
                }
            }
        }
    }

    fn render_main(&self) -> Rendered {
        let visible_todos: Vec<_> = match self.filter {
            Filter::All => self.todos.iter().collect(),
            Filter::Active => self.todos.iter().filter(|todo| !todo.completed).collect(),
            Filter::Completed => self.todos.iter().filter(|todo| todo.completed).collect(),
        };

        html! {
            section.main {
                input #toggle-all.toggle-all type="checkbox";
                label for="toggle-all" { "Mark all as complete" }
                ul.todo-list {
                    @for todo in visible_todos {
                        @let classes = match (todo.completed, todo.editing) {
                            (true, true) => "completed editing",
                            (true, false) => "completed",
                            (false, true) => "editing",
                            (false, false) => "",
                        };
                        li class=(classes) {
                            form
                                phx-submit="edit_todo"
                                method="post"
                                autocapitalize="off"
                                autocomplete="off"
                                autocorrect="off"
                                spellcheck="false"
                                url="#"
                            {
                                div.view {
                                    input.toggle type="checkbox" phx-click="toggle_todo" phx-value-id=(todo.id.to_string()) checked[todo.completed];
                                    label phx-click="toggle_edit_todo" phx-value-id=(todo.id.to_string()) { (todo.title) }
                                    button.destroy phx-click="remove_todo" phx-value-id=(todo.id.to_string()) type="button" {}
                                }
                                input type="hidden" name="id" value=(todo.id.to_string());
                                input.edit name="title" value=(todo.title);
                            }
                        }

                    }
                }
            }
        }
    }

    fn render_footer(&self) -> Rendered {
        let remaining_todos = self.todos.iter().filter(|todo| !todo.completed).count();
        let filter_links = [
            ("All", Filter::All),
            ("Active", Filter::Active),
            ("Completed", Filter::Completed),
        ]
        .into_iter()
        .map(|(label, filter)| (label, filter, filter == self.filter));

        html! {
            section.footer {
                span.todo-count {
                    strong { (remaining_todos) }
                    " item(s) left"
                }

                ul.filters {
                    @for (label, filter, selected) in filter_links {
                        li {
                            @let selected_class = if selected { "selected" } else { "" };
                            @let filter_value = serde_json::to_string(&filter).unwrap();
                            a class=(selected_class) phx-click="set_filter" phx-value-filter=(filter_value.trim_matches('"')) href={"#/" (label)} {
                                (label)
                            }
                        }
                    }
                }

                @if remaining_todos > 0 {
                    button.clear-completed phx-click="clear_completed_todos" { "Clear completed" }
                }
            }

            footer.info {
                p { "Double-click to edit a todo" }
                p {
                    "Created by "
                    a href="https://github.com/tqwewe" { "Ari Seyhun" }
                }
                p {
                    "Part of "
                    a href="https://github.com/lunatic-solutions/submillisecond-live-view" { "Submillisecond Live View" }
                }
            }
        }
    }
}

impl LiveView for Todos {
    type Events = (
        Add,
        Remove,
        Toggle,
        Edit,
        ToggleEdit,
        ClearCompleted,
        SetFilter,
    );

    fn render(&self) -> Rendered {
        html! {
            section.todoapp {
                @(self.render_header())

                @if !self.todos.is_empty() {
                    @(self.render_main())
                    @(self.render_footer())
                }
            }
        }
    }

    fn mount(_uri: Uri) -> Self {
        Todos {
            filter: Filter::All,
            todos: vec![],
        }
    }

    fn styles() -> &'static [&'static str] {
        &["/static/todos.css"]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Todo {
    id: Uuid,
    title: String,
    completed: bool,
    editing: bool,
}

impl Todo {
    fn new(title: String) -> Self {
        Todo {
            id: Uuid::new_v4(),
            title,
            completed: false,
            editing: false,
        }
    }
}

#[derive(Deserialize)]
struct Add {
    title: String,
}

impl LiveViewEvent<Add> for Todos {
    const NAME: &'static str = "add_todo";

    fn handle(state: &mut Self, event: Add, _event_type: String) {
        state.todos.push(Todo::new(event.title));
    }
}

#[derive(Deserialize)]
struct Remove {
    id: Uuid,
}

impl LiveViewEvent<Remove> for Todos {
    const NAME: &'static str = "remove_todo";

    fn handle(state: &mut Self, event: Remove, _event_type: String) {
        state.todos.retain(|todo| todo.id != event.id);
    }
}

#[derive(Deserialize)]
struct Toggle {
    id: Uuid,
    #[serde(default)]
    value: CheckboxValue,
}

impl LiveViewEvent<Toggle> for Todos {
    const NAME: &'static str = "toggle_todo";

    fn handle(state: &mut Self, event: Toggle, _event_type: String) {
        if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id == event.id) {
            todo.completed = event.value.is_checked();
        }
    }
}

#[derive(Deserialize)]
struct Edit {
    id: Uuid,
    title: String,
}

impl LiveViewEvent<Edit> for Todos {
    const NAME: &'static str = "edit_todo";

    fn handle(state: &mut Self, event: Edit, _event_type: String) {
        if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id == event.id) {
            todo.title = event.title;
            todo.editing = false;
        }
    }
}

#[derive(Deserialize)]
struct ToggleEdit {
    id: Uuid,
    detail: u8,
}

impl LiveViewEvent<ToggleEdit> for Todos {
    const NAME: &'static str = "toggle_edit_todo";

    fn handle(state: &mut Self, event: ToggleEdit, _event_type: String) {
        if event.detail == 2 {
            if let Some(todo) = state.todos.iter_mut().find(|todo| todo.id == event.id) {
                todo.editing = true;
            }
        }
    }
}

#[derive(Deserialize)]
struct ClearCompleted {}

impl LiveViewEvent<ClearCompleted> for Todos {
    const NAME: &'static str = "clear_completed_todos";

    fn handle(state: &mut Self, _event: ClearCompleted, _event_type: String) {
        state.todos.retain(|todo| !todo.completed);
    }
}

#[derive(Deserialize)]
struct SetFilter {
    filter: Filter,
}

impl LiveViewEvent<SetFilter> for Todos {
    const NAME: &'static str = "set_filter";

    fn handle(state: &mut Self, event: SetFilter, _event_type: String) {
        state.filter = event.filter;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Filter {
    All,
    Active,
    Completed,
}
