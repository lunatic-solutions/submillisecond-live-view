use maud::html;
use pretty_assertions::assert_eq;
use submillisecond_live_view::rendered::{
    Dynamic, DynamicItems, DynamicList, Dynamics, Rendered, RenderedListItem,
};
use submillisecond_live_view::{
    DOCTYPE, {self as submillisecond_live_view},
};

#[lunatic::test]
fn basic() {
    let rendered = html! {
        p { "Hello, world!" }
    };

    assert_eq!(rendered.statics, ["<p>Hello, world!</p>"]);
    assert_eq!(rendered.dynamics, Dynamics::Items(DynamicItems(vec![])));
    assert!(rendered.templates.is_empty());
}

#[lunatic::test]
fn dynamic() {
    let rendered = html! {
        (DOCTYPE)
        a href={ ("hey") "/lambda-fairy/maud" } {
            "Hello, world!"
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec![
                "".to_string(),
                "<a href=\"".to_string(),
                "/lambda-fairy/maud\">Hello, world!</a>".to_string()
            ],
            dynamics: Dynamics::Items(DynamicItems(vec![
                Dynamic::String("<!DOCTYPE html>".to_string()),
                Dynamic::String("hey".to_string())
            ])),
            templates: vec![]
        }
    );
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

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["Welcome ".to_string(), ".".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
            templates: vec![]
        }
    );

    let logged_in = false;
    let rendered = html! {
        "Welcome "
        @if logged_in {
            (logged_in.to_string())
        }
        "."
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["Welcome ".to_string(), ".".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
            templates: vec![]
        }
    );
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

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["Welcome ".to_string(), ".".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["person".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );

    let logged_in = true;
    let rendered = html! {
        "Welcome "
        @if logged_in {
            (logged_in.to_string())
        }
        "."
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["Welcome ".to_string(), ".".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("true".to_string())])),
                templates: vec![],
            })])),
            templates: vec![],
        }
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

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["Welcome ".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("Bob".to_string())])),
                templates: vec![],
            })])),
            templates: vec![],
        }
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

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["Welcome ".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["stranger".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn if_statement_nested() {
    let render = |count: usize| {
        html! {
            @if count >= 1 {
                p { "Count is high" }
                @if count >= 2 {
                    p { "Count is very high!" }
                }
            }
        }
    };

    let rendered = render(0);

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
            templates: vec![],
        }
    );

    let rendered = render(1);

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );

    let rendered = render(2);

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["<p>Count is high</p>".to_string(), "".to_string()],
                dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                    statics: vec!["<p>Count is very high!</p>".to_string()],
                    dynamics: Dynamics::Items(DynamicItems(vec![])),
                    templates: vec![],
                })])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn for_loop_empty() {
    #[allow(clippy::reversed_empty_ranges)]
    let rendered = html! {
        span { "Hello" }
        @for _ in 0..0 {
            span { "Hi!" }
        }
        span { "world" }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec![
                "<span>Hello</span>".to_string(),
                "<span>world</span>".to_string()
            ],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::String("".to_string())])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn for_loop_statics() {
    let rendered = html! {
        @for _ in 0..3 {
            span { "Hi!" }
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["<span>Hi!</span>".to_string()],
                dynamics: Dynamics::List(DynamicList(vec![vec![], vec![], vec![]])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn for_loop_dynamics() {
    let names = ["John", "Joe", "Jim"];
    let rendered = html! {
        @for name in names {
            span { (name) }
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec!["<span>".to_string(), "</span>".to_string()],
                dynamics: Dynamics::List(DynamicList(vec![
                    vec![Dynamic::String("John".to_string())],
                    vec![Dynamic::String("Joe".to_string())],
                    vec![Dynamic::String("Jim".to_string())],
                ])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );

    let names = ["John", "Joe", "Jim"];
    let rendered = html! {
        @for name in names {
            span class=(name) { (name) }
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec![
                    "<span class=\"".to_string(),
                    "\">".to_string(),
                    "</span>".to_string()
                ],
                dynamics: Dynamics::List(DynamicList(vec![
                    vec![
                        Dynamic::String("John".to_string()),
                        Dynamic::String("John".to_string())
                    ],
                    vec![
                        Dynamic::String("Joe".to_string()),
                        Dynamic::String("Joe".to_string())
                    ],
                    vec![
                        Dynamic::String("Jim".to_string()),
                        Dynamic::String("Jim".to_string())
                    ],
                ])),
                templates: vec![],
            })])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn for_loop_multiple() {
    #[allow(clippy::reversed_empty_ranges)]
    let rendered = html! {
        span { "Hello" }
        @for _ in 0..2 {
            span { "A" }
        }
        @for _ in 0..0 {
            span { "B" }
        }
        span { "world" }
    };

    dbg!(&rendered);

    assert_eq!(
        rendered,
        Rendered {
            statics: vec![
                "<span>Hello</span>".to_string(),
                "".to_string(),
                "<span>world</span>".to_string()
            ],
            dynamics: Dynamics::Items(DynamicItems(vec![
                Dynamic::Nested(Rendered {
                    statics: vec!["<span>A</span>".to_string()],
                    dynamics: Dynamics::List(DynamicList(vec![vec![], vec![]])),
                    templates: vec![]
                }),
                Dynamic::String("".to_string()),
            ])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn for_loop_with_if() {
    let names = ["John", "Joe", "Jim"];
    let rendered = html! {
        @for name in names {
            span { "Welcome, " (name) "." }
            @if name == "Jim" {
                span { "You are a VIP, " (name.to_lowercase()) }
            }
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec![
                    "<span>Welcome, ".to_string(),
                    ".</span>".to_string(),
                    "".to_string()
                ],
                dynamics: Dynamics::List(DynamicList(vec![
                    vec![
                        Dynamic::String("John".to_string()),
                        Dynamic::String("".to_string()),
                    ],
                    vec![
                        Dynamic::String("Joe".to_string()),
                        Dynamic::String("".to_string()),
                    ],
                    vec![
                        Dynamic::String("Jim".to_string()),
                        Dynamic::Nested(RenderedListItem {
                            statics: 0,
                            dynamics: vec![Dynamic::String("jim".to_string())],
                        })
                    ],
                ])),
                templates: vec![vec![
                    "<span>You are a VIP, ".to_string(),
                    "</span>".to_string()
                ]],
            })])),
            templates: vec![]
        }
    );
}

#[lunatic::test]
fn for_loop_with_multiple_ifs() {
    let names = ["John", "Joe", "Jim"];
    let rendered = html! {
        @for name in names {
            span { "Welcome, " (name) "." }
            @if name == "Jim" {
                span { "You are a VIP, " (name.to_lowercase()) }
                @if name.ends_with('m') {
                    span { (name) " ends with m" }
                }
            }
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec![
                    "<span>Welcome, ".to_string(),
                    ".</span>".to_string(),
                    "".to_string()
                ],
                dynamics: Dynamics::List(DynamicList(vec![
                    vec![
                        Dynamic::String("John".to_string()),
                        Dynamic::String("".to_string()),
                    ],
                    vec![
                        Dynamic::String("Joe".to_string()),
                        Dynamic::String("".to_string()),
                    ],
                    vec![
                        Dynamic::String("Jim".to_string()),
                        Dynamic::Nested(RenderedListItem {
                            statics: 1,
                            dynamics: vec![
                                Dynamic::String("jim".to_string()),
                                Dynamic::Nested(RenderedListItem {
                                    statics: 0,
                                    dynamics: vec![Dynamic::String("Jim".to_string())],
                                }),
                            ],
                        })
                    ],
                ])),
                templates: vec![
                    vec!["<span>".to_string(), " ends with m</span>".to_string()],
                    vec![
                        "<span>You are a VIP, ".to_string(),
                        "</span>".to_string(),
                        "".to_string()
                    ],
                ],
            })])),
            templates: vec![],
        }
    );
}

#[lunatic::test]
fn for_loop_with_many_ifs() {
    let names = ["John", "Joe", "Jim"];
    let rendered = html! {
        @for name in names {
            span { "Welcome, " (name) "." }
            @if name == "Jim" || name == "Joe" {
                span { "You are a VIP, " (name.to_lowercase()) }
                @if name.ends_with('m') || name.ends_with('e') {
                    span { (name) " ends with m or e" }
                }
            }
        }
    };

    assert_eq!(
        rendered,
        Rendered {
            statics: vec!["".to_string(), "".to_string()],
            dynamics: Dynamics::Items(DynamicItems(vec![Dynamic::Nested(Rendered {
                statics: vec![
                    "<span>Welcome, ".to_string(),
                    ".</span>".to_string(),
                    "".to_string()
                ],
                dynamics: Dynamics::List(DynamicList(vec![
                    vec![
                        Dynamic::String("John".to_string()),
                        Dynamic::String("".to_string()),
                    ],
                    vec![
                        Dynamic::String("Joe".to_string()),
                        Dynamic::Nested(RenderedListItem {
                            statics: 1,
                            dynamics: vec![
                                Dynamic::String("joe".to_string()),
                                Dynamic::Nested(RenderedListItem {
                                    statics: 0,
                                    dynamics: vec![Dynamic::String("Joe".to_string())],
                                }),
                            ],
                        }),
                    ],
                    vec![
                        Dynamic::String("Jim".to_string()),
                        Dynamic::Nested(RenderedListItem {
                            statics: 1,
                            dynamics: vec![
                                Dynamic::String("jim".to_string()),
                                Dynamic::Nested(RenderedListItem {
                                    statics: 0,
                                    dynamics: vec![Dynamic::String("Jim".to_string())],
                                }),
                            ],
                        }),
                    ],
                ])),
                templates: vec![
                    vec!["<span>".to_string(), " ends with m or e</span>".to_string()],
                    vec![
                        "<span>You are a VIP, ".to_string(),
                        "</span>".to_string(),
                        "".to_string()
                    ],
                ],
            })])),
            templates: vec![]
        }
    );
}
