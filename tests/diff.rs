use serde_json::json;
use submillisecond_live_view::html;

#[lunatic::test]
fn dynamic_diff() {
    let render = |s: &str| {
        html! {
            a href={ (s) "/lambda-fairy/maud" } {
                "Hello, world!"
            }
        }
    };

    let diff = render("hey").diff(render("there"));
    assert_eq!(
        diff,
        Some(json!({
            "0": "there"
        }))
    );
}

#[lunatic::test]
fn if_statement_false_to_true_diff() {
    let render = |logged_in: bool| {
        html! {
            "Welcome "
            @if logged_in {
                "person"
            }
            "."
        }
    };

    let diff = render(false).diff(render(true));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "s": [
                    "person"
                ]
            }
        }))
    );

    let render = |logged_in: bool| {
        html! {
            "Welcome "
            @if logged_in {
                (logged_in.to_string())
            }
            "."
        }
    };

    let diff = render(false).diff(render(true));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "0": "true",
                "s": [
                    "",
                    ""
                ]
            }
        }))
    );
}

#[lunatic::test]
fn if_statement_true_to_false_diff() {
    let render = |logged_in: bool| {
        html! {
            "Welcome "
            @if logged_in {
                "person"
            }
            "."
        }
    };

    let diff = render(true).diff(render(false));
    assert_eq!(
        diff,
        Some(json!({
            "0": ""
        }))
    );

    let render = |logged_in: bool| {
        html! {
            "Welcome "
            @if logged_in {
                (logged_in.to_string())
            }
            "."
        }
    };

    let diff = render(true).diff(render(false));
    assert_eq!(
        diff,
        Some(json!({
            "0": ""
        }))
    );
}

#[lunatic::test]
fn if_statement_let_none_to_some_diff() {
    let render = |user: Option<&str>| {
        html! {
            "Welcome "
            @if let Some(user) = user {
                (user)
            } @else {
                "stranger"
            }
        }
    };

    let diff = render(None).diff(render(Some("Bob")));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "0": "Bob",
                "s": [
                    "",
                    ""
                ]
            }
        }))
    );
}

#[lunatic::test]
fn if_statement_let_some_to_none_diff() {
    let render = |user: Option<&str>| {
        html! {
            "Welcome "
            @if let Some(user) = user {
                (user)
            } @else {
                "stranger"
            }
        }
    };

    let diff = render(Some("Bob")).diff(render(None));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "s": [
                    "stranger"
                ]
            }
        }))
    );
}

#[lunatic::test]
fn if_statement_nested_diff() {
    let render = |count: i32| {
        html! {
            @if count >= 1 {
                p { "Count is high" }
                @if count >= 2 {
                    p { "Count is very high!" }
                }
            }
        }
    };

    let diff = render(0).diff(render(1));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "0": "",
                "s": [
                    "<p>Count is high</p>",
                    ""
                ]
            }
        }))
    );

    let diff = render(1).diff(render(2));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "0": {
                    "s": [
                        "<p>Count is very high!</p>"
                    ]
                }
            }
        }))
    );

    let diff = render(2).diff(render(3));
    assert_eq!(diff, None);
}

#[lunatic::test]
fn for_loop_statics() {
    let render = |names: &[&str]| {
        html! {
            @for _ in names {
                span { "Hi" }
            }
        }
    };

    let diff = render(&[]).diff(render(&["John"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    []
                ],
                "s": [
                    "<span>Hi</span>"
                ]
            }
        }))
    );

    let diff = render(&["John"]).diff(render(&["John", "Jim"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    [],
                    []
                ]
            }
        }))
    );

    let diff = render(&["John", "Jim"]).diff(render(&["John"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    []
                ]
            }
        }))
    );

    let diff = render(&["John"]).diff(render(&[]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": []
            }
        }))
    );
}

#[lunatic::test]
fn for_loop_dynamics() {
    let render = |names: &[&str]| {
        html! {
            @for name in names {
                span { (name) }
            }
        }
    };

    let diff = render(&[]).diff(render(&["John"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    [
                        "John"
                    ]
                ],
                "s": [
                    "<span>",
                    "</span>"
                ]
            }
        }))
    );

    let diff = render(&["John"]).diff(render(&["John", "Joe"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    [
                        "John"
                    ],
                    [
                        "Joe"
                    ]
                ]
            }
        }))
    );

    let diff = render(&["John", "Joe"]).diff(render(&["John", "Joe", "Jim"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    [
                        "John"
                    ],
                    [
                        "Joe"
                    ],
                    [
                        "Jim"
                    ]
                ]
            }
        }))
    );

    let diff = render(&["John", "Joe"]).diff(render(&["John"]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": [
                    [
                        "John"
                    ]
                ]
            }
        }))
    );

    let diff = render(&["John"]).diff(render(&[]));
    assert_eq!(
        diff,
        Some(json!({
            "0": {
                "d": []
            }
        }))
    );
}
