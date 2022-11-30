use std::borrow::Cow;
use std::env;
use std::marker::PhantomData;

pub use ::maud_live_view::*;
use hmac::{Hmac, Mac};
use jwt::VerifyWithKey;
use lunatic::process::ProcessRef;
use lunatic_log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use submillisecond::http::Uri;
use submillisecond::response::Response;
use submillisecond::RequestContext;
use thiserror::Error;

use crate::manager::{Join, LiveViewManager, LiveViewManagerResult};
use crate::rendered::{IntoJson, Rendered};
use crate::socket::{Event, JoinEvent, Socket};
use crate::template::{TemplateProcess, TemplateProcessHandler};
use crate::LiveView;

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct LiveViewMaud<T> {
    phantom: PhantomData<T>,
    template_process: ProcessRef<TemplateProcess>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Session {
    pub(crate) csrf_token: String,
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

impl<T> LiveViewMaud<T> {
    pub(crate) fn new(template_process: ProcessRef<TemplateProcess>) -> Self {
        LiveViewMaud {
            phantom: PhantomData,
            template_process,
        }
    }
}

impl<T> Clone for LiveViewMaud<T> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
            template_process: self.template_process.clone(),
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
        let content = T::mount(req.uri().clone(), None).render().to_string();
        let html = self.template_process.render(content);

        Response::builder()
            .header("Content-Type", "text/html; charset=UTF-8")
            .body(html.into_bytes())
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

#[cfg(debug_assertions)]
const SECRET_DEFAULT: [u8; 32] = *b"liveview-debug-secret-csrf-token";

#[cfg(not(debug_assertions))]
const SECRET_DEFAULT: [u8; 32] = const_random::const_random!([u8; 32]);

pub(crate) fn secret() -> Cow<'static, [u8]> {
    match env::var("LIVE_VIEW_SECRET") {
        Ok(secret) => Cow::Owned(secret.into_bytes()),
        Err(_) => Cow::Borrowed(&SECRET_DEFAULT),
    }
}
