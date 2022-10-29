use std::collections::HashMap;

use submillisecond_live_view::html;
use submillisecond_live_view::rendered::{DiffRender, Dynamic, Rendered, RenderedDiff};

fn render_diff(a: Rendered, b: Rendered) -> RenderedDiff {
    a.diff(b)
}

#[lunatic::test]
fn dynamic_diff() {
    let render = |s: &str| {
        html! {
            a href={ (s) "/lambda-fairy/maud" } {
                "Hello, world!"
            }
        }
    };

    let diff = render_diff(render("hey"), render("there"));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(0, Dynamic::String("there".to_string()))]),
        }
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
    let diff = render_diff(render(false), render(true));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(
                0,
                Dynamic::Nested(RenderedDiff {
                    statics: vec!["person".to_string()],
                    dynamics: HashMap::default()
                })
            )])
        }
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
    let diff = render_diff(render(false), render(true));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(
                0,
                Dynamic::Nested(RenderedDiff {
                    statics: vec!["".to_string(), "".to_string()],
                    dynamics: HashMap::from_iter([(0, Dynamic::String("true".to_string()))]),
                })
            )])
        }
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
    let diff = render_diff(render(true), render(false));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(0, Dynamic::String("".to_string()))])
        }
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
    let diff = render_diff(render(true), render(false));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(0, Dynamic::String("".to_string()))])
        }
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
    let diff = render_diff(render(None), render(Some("Bob")));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(
                0,
                Dynamic::Nested(RenderedDiff {
                    statics: vec!["".to_string(), "".to_string()],
                    dynamics: HashMap::from_iter([(0, Dynamic::String("Bob".to_string()))]),
                })
            )])
        }
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
    let diff = render_diff(render(Some("Bob")), render(None));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(
                0,
                Dynamic::Nested(RenderedDiff {
                    statics: vec!["stranger".to_string()],
                    dynamics: HashMap::new(),
                })
            )])
        }
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
    let diff = render_diff(render(0), render(1));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(
                0,
                Dynamic::Nested(RenderedDiff {
                    statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
                    dynamics: HashMap::from_iter([(0, Dynamic::String("".to_string()))]),
                })
            )])
        }
    );

    let diff = render_diff(render(1), render(2));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::from_iter([(
                0,
                Dynamic::Nested(RenderedDiff {
                    statics: vec![],
                    dynamics: HashMap::from_iter([(
                        0,
                        Dynamic::Nested(RenderedDiff {
                            statics: vec!["<p>Count is very high!</p>".to_string()],
                            dynamics: HashMap::default(),
                        })
                    )]),
                })
            )])
        }
    );

    let diff = render_diff(render(2), render(3));
    assert_eq!(
        diff,
        RenderedDiff {
            statics: vec![],
            dynamics: HashMap::new(),
        }
    );
}
