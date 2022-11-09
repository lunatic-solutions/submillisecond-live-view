use std::borrow::Cow;
use std::env;
use std::marker::PhantomData;

pub use ::maud_live_view::*;
use const_random::const_random;
use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use lunatic_log::error;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use submillisecond::http::Uri;
use submillisecond::response::Response;
use submillisecond::RequestContext;
use thiserror::Error;

use crate::csrf::CsrfToken;
use crate::head::{Script, Style};
use crate::manager::{Join, LiveViewManager, LiveViewManagerResult};
use crate::rendered::{IntoJson, Rendered};
use crate::socket::{Event, JoinEvent, Socket};
use crate::{self as submillisecond_live_view, html, LiveView};

#[cfg(all(debug_assertions, feature = "liveview_js"))]
const LIVEVIEW_JS: &str = include_str!("../liveview-debug.js");

#[cfg(all(not(debug_assertions), feature = "liveview_js"))]
const LIVEVIEW_JS: &str = include_str!("../liveview-release.js");

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct LiveViewMaud<T> {
    phantom: PhantomData<T>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Session {
    csrf_token: String,
}

#[derive(Clone, Copy, Debug, Error, Serialize, Deserialize)]
pub(crate) enum LiveViewMaudError {
    #[error("invalid csrf token")]
    InvalidCsrfToken,
    #[error("invalid url")]
    InvalidUrl,
    #[error("missing url")]
    MissingUrl,
}

impl<T> Clone for LiveViewMaud<T> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
        }
    }
}

impl<T> Copy for LiveViewMaud<T> {}

impl<T> Default for LiveViewMaud<T> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T> LiveViewManager<T> for LiveViewMaud<T>
where
    T: LiveView,
{
    type State = Rendered;
    // type Reply = Value;
    type Error = LiveViewMaudError;

    fn handle_request(&self, req: RequestContext) -> Response {
        let mut rng = rand::thread_rng();
        let id: String = (&mut rng)
            .sample_iter(Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        let key: Hmac<Sha256> = Hmac::new_from_slice(&secret()).expect("unable to encode secret");

        let csrf_token = CsrfToken::generate().masked;
        let session = Session {
            csrf_token: csrf_token.clone(),
        };
        let session_str = session.sign_with_key(&key).expect("failed to sign session");

        let content = T::mount(req.uri().clone(), None).render().to_string();

        let head = T::head();

        let body = html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    title { (head.title) }
                    meta name="csrf-token" content=(csrf_token);
                    @for meta in &head.meta {
                        // Dynamic attributes aren't supported yet.
                        // See <https://github.com/lambda-fairy/maud/issues/240>
                        @let attrs = meta.attrs.iter().map(|attr| format!("{}=\"{}\"", attr.name, attr.value)).collect::<Vec<_>>().join(" ");
                        (PreEscaped(format!("<meta {attrs}>")))
                    }
                    @for style in head.styles {
                        @match style {
                            Style::Link(href) => link rel="stylesheet" href=(href);,
                            Style::Css(css) => style { (PreEscaped(css)) },
                        }
                    }
                    @for script in head.scripts {
                        @match script {
                            Script::Link { href, defer } => script defer[defer] type="text/javascript" src=(href) {},
                            Script::Js(js) => script type="text/javascript" { (PreEscaped(js)) },
                            #[cfg(feature = "liveview_js")]
                            Script::LiveView => script type="text/javascript" { (PreEscaped(LIVEVIEW_JS)) },
                        }
                    }
                }
                body {
                    div data-phx-main="true" data-phx-static="" data-phx-session=(session_str) id=(id) {
                        (PreEscaped(content))
                    }
                }
            }
        };

        Response::builder()
            .header("Content-Type", "text/html; charset=UTF-8")
            .body(body.to_string().into_bytes())
            .unwrap()
    }

    fn handle_join(
        &self,
        socket: Socket,
        event: JoinEvent,
    ) -> LiveViewManagerResult<Join<T, Self::State, Value>, Self::Error> {
        let key: Hmac<Sha256> = Hmac::new_from_slice(&secret()).expect("unable to encode secret");
        let session: Result<Session, _> = event.session.verify_with_key(&key);

        // Verify csrf token
        if !session
            .map(|session| session.csrf_token == event.params.csrf_token)
            .unwrap_or(false)
        {
            return LiveViewManagerResult::FatalError(LiveViewMaudError::InvalidCsrfToken);
        }

        macro_rules! tri_fatal {
            ($e: expr) => {
                match $e {
                    Result::Ok(ok) => ok,
                    Err(err) => {
                        return LiveViewManagerResult::FatalError(err);
                    }
                }
            };
        }

        let uri: Uri = tri_fatal!(tri_fatal!(event.url().ok_or(LiveViewMaudError::MissingUrl))
            .parse()
            .map_err(|_| LiveViewMaudError::InvalidUrl));

        let live_view = T::mount(uri, Some(socket));
        let state = live_view.render();
        let reply = state.clone().into_json();
        LiveViewManagerResult::Ok(Join {
            live_view,
            state,
            reply,
        })
    }

    fn handle_event(
        &self,
        _event: Event,
        state: &mut Self::State,
        live_view: &T,
    ) -> LiveViewManagerResult<Option<Value>, Self::Error> {
        let rendered = live_view.render();
        let diff = state.clone().diff(rendered.clone()); // TODO: Remove these clones
        *state = rendered;

        LiveViewManagerResult::Ok(diff)
    }
}

const SECRET_DEFAULT: [u8; 32] = const_random!([u8; 32]);

fn secret() -> Cow<'static, [u8]> {
    match env::var("LIVE_VIEW_SECRET") {
        Ok(secret) => Cow::Owned(secret.into_bytes()),
        Err(_) => Cow::Borrowed(&SECRET_DEFAULT),
    }
}
