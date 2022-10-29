use maud::html;
use submillisecond_live_view::rendered::{Dynamic, Rendered};
use submillisecond_live_view::{self as submillisecond_live_view};

#[lunatic::test]
fn basic() {
    let rendered = html! {
        p { "Hello, world!" }
    };

    assert_eq!(rendered.statics, ["<p>Hello, world!</p>"]);
    assert_eq!(rendered.dynamics, []);
}

#[lunatic::test]
fn dynamic() {
    let rendered = html! {
        a href={ ("hey") "/lambda-fairy/maud" } {
            "Hello, world!"
        }
    };

    assert_eq!(
        rendered.statics,
        ["<a href=\"", "/lambda-fairy/maud\">Hello, world!</a>"]
    );
    assert_eq!(rendered.dynamics, [Dynamic::String("hey".to_string())]);
}

#[lunatic::test]
fn if_statement_false() {
    let logged_in = false;
    let rendered = html! {
        "Welcome "
        @if logged_in {
            "person"
        }
        "."
    };

    assert_eq!(rendered.statics, ["Welcome ", "."]);
    assert_eq!(rendered.dynamics, [Dynamic::String("".to_string())]);

    let logged_in = false;
    let rendered = html! {
        "Welcome "
        @if logged_in {
            (logged_in.to_string())
        }
        "."
    };

    assert_eq!(rendered.statics, ["Welcome ", "."]);
    assert_eq!(rendered.dynamics, [Dynamic::String("".to_string())]);
}

#[lunatic::test]
fn if_statement_true() {
    let logged_in = true;
    let rendered = html! {
        "Welcome "
        @if logged_in {
            "person"
        }
        "."
    };

    assert_eq!(rendered.statics, ["Welcome ", "."]);
    assert_eq!(
        rendered.dynamics,
        [Dynamic::Nested(Rendered {
            statics: vec!["person".to_string()],
            dynamics: vec![]
        })]
    );

    let logged_in = true;
    let rendered = html! {
        "Welcome "
        @if logged_in {
            (logged_in.to_string())
        }
        "."
    };

    assert_eq!(rendered.statics, ["Welcome ", "."]);
    assert_eq!(
        rendered.dynamics,
        [Dynamic::Nested(Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: vec![Dynamic::String("true".to_string())]
        })]
    );
}

#[lunatic::test]
fn if_statement_let_some() {
    let user = Some("Bob");
    let rendered = html! {
        "Welcome "
        @if let Some(user) = user {
            (user)
        } @else {
            "stranger"
        }
    };

    assert_eq!(rendered.statics, ["Welcome ", ""]);
    assert_eq!(
        rendered.dynamics,
        [Dynamic::Nested(Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: vec![Dynamic::String("Bob".to_string())]
        })]
    );
}

#[lunatic::test]
fn if_statement_let_none() {
    let user: Option<&str> = None;
    let rendered = html! {
        "Welcome "
        @if let Some(user) = user {
            (user)
        } @else {
            "stranger"
        }
    };

    assert_eq!(rendered.statics, ["Welcome ", ""]);
    assert_eq!(
        rendered.dynamics,
        [Dynamic::Nested(Rendered {
            statics: vec!["stranger".to_string()],
            dynamics: vec![]
        })]
    );
}

#[lunatic::test]
fn if_statement_nested() {
    let count = 0;
    let rendered = html! {
        @if count >= 1 {
            p { "Count is high" }
            @if count >= 2 {
                p { "Count is very high!" }
            }
        }
    };

    assert_eq!(rendered.statics, ["", ""]);
    assert_eq!(rendered.dynamics, [Dynamic::String("".to_string())]);

    let count = 1;
    let rendered = html! {
        @if count >= 1 {
            p { "Count is high" }
            @if count >= 2 {
                p { "Count is very high!" }
            }
        }
    };

    assert_eq!(rendered.statics, ["", ""]);
    assert_eq!(
        rendered.dynamics,
        [Dynamic::Nested(Rendered {
            statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
            dynamics: vec![Dynamic::String("".to_string())]
        })]
    );

    let count = 2;
    let rendered = html! {
        @if count >= 1 {
            p { "Count is high" }
            @if count >= 2 {
                p { "Count is very high!" }
            }
        }
    };

    assert_eq!(rendered.statics, ["", ""]);
    assert_eq!(
        rendered.dynamics,
        [Dynamic::Nested(Rendered {
            statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
            dynamics: vec![Dynamic::Nested(Rendered {
                statics: vec!["<p>Count is very high!</p>".to_string()],
                dynamics: vec![]
            })]
        })]
    );
}
