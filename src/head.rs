/// Html head containing title, meta, styles, and scripts.
pub struct Head {
    pub title: &'static str,
    pub meta: Vec<Meta>,
    pub styles: Vec<Style>,
    pub scripts: Vec<Script>,
}

/// Html meta tag.
pub struct Meta {
    pub attrs: Vec<MetaAttr>,
}

/// Html meta attribute.
pub struct MetaAttr {
    pub name: &'static str,
    pub value: &'static str,
}

/// Style element.
pub enum Style {
    /// Link to external css styles.
    Link(&'static str),
    /// Embedded CSS placed in a style tag.
    Css(&'static str),
}

/// Script element.
pub enum Script {
    /// Link to an external script.
    Link { href: &'static str, defer: bool },
    /// Embedded JavaScript placed in a script tag.
    Js(&'static str),
    #[cfg(feature = "liveview_js")]
    /// Liveview embedded JS.
    LiveView,
}

impl Head {
    /// Create new empty head with a page title.
    pub fn new(title: &'static str) -> Self {
        Head {
            title,
            meta: vec![],
            styles: vec![],
            scripts: vec![],
        }
    }

    /// Create new head with defaults.
    pub fn defaults() -> Self {
        #[allow(unused_mut)]
        let mut head = Head::new(env!("CARGO_PKG_NAME"))
            .with_meta(Meta {
                attrs: vec![MetaAttr {
                    name: "charset",
                    value: "utf-8",
                }],
            })
            .with_meta(Meta {
                attrs: vec![
                    MetaAttr {
                        name: "http-equiv",
                        value: "X-UA-Compatible",
                    },
                    MetaAttr {
                        name: "content",
                        value: "IE=edge",
                    },
                ],
            })
            .with_meta(Meta {
                attrs: vec![MetaAttr {
                    name: "viewport",
                    value: "width=device-width, initial-scale=1.0",
                }],
            });

        #[cfg(feature = "liveview_js")]
        {
            head = head.with_script(Script::LiveView);
        }

        #[allow(clippy::let_and_return)]
        head
    }

    /// Set the page title.
    pub fn with_title(mut self, title: &'static str) -> Self {
        self.title = title;
        self
    }

    /// Add a meta element.
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta.push(meta);
        self
    }

    /// Add a style element.
    pub fn with_style(mut self, style: Style) -> Self {
        self.styles.push(style);
        self
    }

    /// Add a script element.
    pub fn with_script(mut self, script: Script) -> Self {
        self.scripts.push(script);
        self
    }
}
