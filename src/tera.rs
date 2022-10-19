mod rendered;
mod rendered_json;

use std::marker::PhantomData;
use std::path::PathBuf;

use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use lunatic::abstract_process;
use lunatic::process::{ProcessRef, StartProcess};
use lunatic_log::error;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use submillisecond::http::StatusCode;
use submillisecond::response::{IntoResponse, Response};
use submillisecond::RequestContext;
use tera::{Context, Tera};
use thiserror::Error;

use self::rendered::Rendered;
use crate::csrf::CsrfToken;
use crate::handler::LiveViewHandler;
use crate::manager::{LiveViewManager, LiveViewManagerResult};
use crate::socket::{Event, JoinEvent};
use crate::tera::rendered_json::RenderedJson;
use crate::LiveView;

const LIVE_VIEW_CONTEXT_ID: &str = "live_view_context-699a5452-a8c9-413e-a77f-068736b37783";

#[derive(Clone, Serialize, Deserialize)]
pub struct LiveViewTera<T> {
    template_process: ProcessRef<LiveViewTeraRenderer<T>>,
}

impl<T> LiveViewTera<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    /// Register a template with a live view.
    pub fn route(template: &'static str) -> LiveViewHandler<Self, T> {
        let live_view_context = ProcessRef::<LiveViewContext>::lookup(LIVE_VIEW_CONTEXT_ID)
            .expect("live view context not initialized");
        let layout = live_view_context.layout();
        let process_name = format!(
            "{}-{}-{}",
            std::any::type_name::<T>(),
            layout.to_string_lossy(),
            template
        );

        let template_process = match ProcessRef::lookup(&process_name) {
            Some(template_process) => template_process,
            None => LiveViewTeraRenderer::start((layout, template.into()), Some(&process_name)),
        };

        LiveViewHandler::new(LiveViewTera { template_process })
    }
}

impl<T> LiveViewManager<T> for LiveViewTera<T>
where
    T: LiveView + Clone + Serialize + for<'de> Deserialize<'de>,
{
    type State = RenderedJson;
    type Reply = serde_json::Value;
    type Error = LiveViewTeraError;

    fn handle_request(&self, _req: RequestContext) -> Response {
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
        let token_str = session.sign_with_key(&key).expect("failed to sign session");

        match self
            .template_process
            .render_static(T::mount(None), csrf_token, id, token_str)
        {
            Ok(body) => Response::builder()
                .header("Content-Type", "text/html; charset=UTF-8")
                .body(body.into_bytes())
                .unwrap(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }

    fn handle_join(
        &self,
        event: JoinEvent,
        values: &T,
    ) -> LiveViewManagerResult<(Self::State, Self::Reply), Self::Error> {
        let live_view_context = ProcessRef::<LiveViewContext>::lookup(LIVE_VIEW_CONTEXT_ID)
            .expect("live view context not initialized");
        let secret = live_view_context.secret();

        let key: Hmac<Sha256> = Hmac::new_from_slice(&secret).expect("unable to encode secret");
        let session: Session = event.session.verify_with_key(&key).expect("nope!");

        // Verify csrf token
        if session.csrf_token != event.params.csrf_token {
            return LiveViewManagerResult::FatalError(LiveViewTeraError::InvalidCsrfToken);
        }

        let rendered = self
            .template_process
            .render_dynamic(values.clone())
            .expect("failed to render template");
        let state = RenderedJson::from(rendered.clone());
        let reply = json!({ "rendered": RenderedJson::from(rendered) });
        LiveViewManagerResult::Ok((state, reply))
    }

    fn handle_event(
        &self,
        state: &mut Self::State,
        _event: Event,
        values: &T,
    ) -> LiveViewManagerResult<Self::Reply, Self::Error> {
        let rendered = RenderedJson::from(
            self.template_process
                .render_dynamic(values.clone())
                .expect("failed to render template"),
        );

        let diff = state.diff(&rendered);
        *state = rendered;

        LiveViewManagerResult::Ok(json!({ "diff": diff }))
    }
}

#[derive(Serialize, Deserialize)]
pub struct LiveViewContext {
    secret: Vec<u8>,
    layout: PathBuf,
}

impl LiveViewContext {
    pub fn init(secret: &[u8], layout: impl Into<PathBuf>) -> ProcessRef<LiveViewContext> {
        LiveViewContext::start(
            LiveViewContext {
                secret: secret.into(),
                layout: layout.into(),
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

    #[handle_request]
    fn layout(&self) -> PathBuf {
        self.layout.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Session {
    csrf_token: String,
}

#[derive(Debug, Error)]
pub enum LiveViewTeraError {
    #[error("invalid csrf token")]
    InvalidCsrfToken,
}

struct LiveViewTeraRenderer<T> {
    tera: Tera,
    data: PhantomData<T>,
}

#[abstract_process]
impl<T> LiveViewTeraRenderer<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    #[init]
    fn init(_: ProcessRef<Self>, (layout, path): (PathBuf, PathBuf)) -> Self {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![]);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("could not extract file name from template");
        tera.add_template_file(layout, Some("__layout"))
            .expect("unable to add layout file");
        tera.add_template_file(&path, Some(name))
            .expect("unable to add template file");

        LiveViewTeraRenderer {
            tera,
            data: PhantomData::default(),
        }
    }

    #[handle_request]
    fn render_static(
        &self,
        value: T,
        csrf_token: String,
        id: String,
        session: String,
    ) -> Result<String, String> {
        let context = Context::from_serialize(value).map_err(|err| err.to_string())?;
        let name = self
            .tera
            .templates
            .keys()
            .find(|name| *name != "__layout")
            .expect("template does not exist");
        let content = self
            .tera
            .render(name, &context)
            .map_err(|err| err.to_string())?;

        let mut context = Context::new();
        context.insert("csrf_token", &csrf_token);
        context.insert(
            "inner_content",
            &format!(r#"<div data-phx-main="true" data-phx-static="" data-phx-session="{session}" id="{id}">{content}</div>"#),
        );
        self.tera
            .render("__layout", &context)
            .map_err(|err| err.to_string())
    }

    #[handle_request]
    fn render_dynamic(&self, value: T) -> Result<Rendered, String> {
        let context = Context::from_serialize(value).map_err(|err| err.to_string())?;
        let name = self
            .tera
            .templates
            .keys()
            .find(|name| *name != "__layout")
            .expect("template does not exist");
        let mut rendered = Rendered::default();
        self.tera
            .render_to(name, &context, &mut rendered)
            .map_err(|err| err.to_string())?;

        Ok(rendered)
    }
}
