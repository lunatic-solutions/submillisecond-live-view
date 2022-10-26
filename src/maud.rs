use std::marker::PhantomData;

use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use lunatic::abstract_process;
use lunatic::process::{ProcessRef, StartProcess};
use lunatic_log::error;
use maud::{html, PreEscaped, DOCTYPE};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use submillisecond::http::Uri;
use submillisecond::response::Response;
use submillisecond::RequestContext;
use thiserror::Error;

use crate::csrf::CsrfToken;
use crate::handler::LiveViewHandler;
use crate::manager::{Join, LiveViewManager, LiveViewManagerResult};
use crate::rendered::{DiffRender, IntoJson, Rendered};
use crate::socket::{Event, JoinEvent};
use crate::{self as submillisecond_live_view, LiveView};

const LIVE_VIEW_CONTEXT_ID: &str = "live_view_context-699a5452-a8c9-413e-a77f-068736b37783";

#[derive(Clone, Serialize, Deserialize)]
pub struct LiveViewMaud<T> {
    phantom: PhantomData<T>,
}

impl<T> LiveViewMaud<T> {
    /// Register a template with a live view.
    pub fn route() -> LiveViewHandler<Self, T> {
        LiveViewHandler::new(LiveViewMaud {
            phantom: PhantomData,
        })
    }
}

impl<T> LiveViewManager<T> for LiveViewMaud<T>
where
    T: LiveView,
{
    type State = Rendered;
    type Reply = Value;
    type Error = LiveViewMaudError;

    fn handle_request(&self, req: RequestContext) -> Response {
        let mut rng = rand::thread_rng();
        let id: String = (&mut rng)
            .sample_iter(Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        let live_view_context = ProcessRef::<LiveViewContext>::lookup(LIVE_VIEW_CONTEXT_ID)
            .expect("live view context not initialized");
        let secret = live_view_context.secret();

        let key: Hmac<Sha256> = Hmac::new_from_slice(&secret).expect("unable to encode secret");

        let csrf_token = CsrfToken::generate().masked;
        let session = Session {
            csrf_token: csrf_token.clone(),
        };
        let session_str = session.sign_with_key(&key).expect("failed to sign session");

        let content = T::mount(req.uri().clone()).render().to_string();

        let body = html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    meta charset="utf-8";
                    meta http-equiv="X-UA-Compatible" content="IE=edge";
                    meta name="viewport" content="width=device-width, initial-scale=1.0";
                    meta name="csrf-token" content=(csrf_token);
                    title { "submillisecond live view" }
                    @for style in T::styles() {
                        link rel="stylesheet" href=(style);
                    }
                    script defer type="text/javascript" src="/static/main.js" {}
                    @for script in T::scripts() {
                        script defer type="text/javascript" src=(script) {}
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
        event: JoinEvent,
    ) -> LiveViewManagerResult<Join<T, Self::State, Self::Reply>, Self::Error> {
        let live_view_context = ProcessRef::<LiveViewContext>::lookup(LIVE_VIEW_CONTEXT_ID)
            .expect("live view context not initialized");
        let secret = live_view_context.secret();

        let key: Hmac<Sha256> = Hmac::new_from_slice(&secret).expect("unable to encode secret");
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

        let live_view = T::mount(uri);
        let state = live_view.render();
        let reply = json!({ "rendered": state.clone().into_json() });
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
    ) -> LiveViewManagerResult<Self::Reply, Self::Error> {
        let rendered = live_view.render();
        let diff = state.clone().diff(rendered.clone()); // TODO: Remove these clones
        *state = rendered;

        LiveViewManagerResult::Ok(json!({ "diff": diff.into_json() }))
    }
}

#[derive(Serialize, Deserialize)]
pub struct LiveViewContext {
    secret: Vec<u8>,
}

impl LiveViewContext {
    pub fn init(secret: &[u8]) -> ProcessRef<LiveViewContext> {
        LiveViewContext::start(
            LiveViewContext {
                secret: secret.into(),
            },
            Some(LIVE_VIEW_CONTEXT_ID),
        )
    }
}

#[abstract_process]
impl LiveViewContext {
    #[init]
    fn initialize(_: ProcessRef<Self>, ctx: Self) -> Self {
        ctx
    }

    #[handle_request]
    fn secret(&self) -> Vec<u8> {
        self.secret.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Session {
    csrf_token: String,
}

#[derive(Debug, Error)]
pub enum LiveViewMaudError {
    #[error("invalid csrf token")]
    InvalidCsrfToken,
    #[error("invalid url")]
    InvalidUrl,
    #[error("missing url")]
    MissingUrl,
}
