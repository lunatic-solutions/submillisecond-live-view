mod rendered;
mod rendered_json;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::PathBuf;

use lunatic::abstract_process;
use lunatic::process::{ProcessRef, StartProcess};
use lunatic_log::error;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use submillisecond::http::StatusCode;
use submillisecond::response::{IntoResponse, Response};
use submillisecond::RequestContext;
use tera::{Context, Tera};
use thiserror::Error;

use self::rendered::Rendered;
use crate::csrf::CsrfToken;
use crate::live_view::{LiveViewSocket, LiveViewSocketResult};
use crate::socket::{Event, JoinEvent};
use crate::tera::rendered_json::RenderedJson;
use crate::{LiveView, LiveViewHandler};

#[derive(Clone, Serialize, Deserialize)]
pub struct LiveViewTera<T> {
    template_process: ProcessRef<LiveViewTeraRenderer<T>>,
}

impl<T> LiveViewTera<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    pub fn route(layout: &'static str, template: &'static str) -> LiveViewHandler<Self, T> {
        let process_name = format!("{}-{}-{}", std::any::type_name::<T>(), layout, template);

        let template_process = match ProcessRef::lookup(&process_name) {
            Some(template_process) => template_process,
            None => {
                LiveViewTeraRenderer::start((layout.into(), template.into()), Some(&process_name))
            }
        };

        LiveViewHandler::new(LiveViewTera { template_process })
    }
}

impl<T> LiveViewSocket<T> for LiveViewTera<T>
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

        match self
            .template_process
            .render_static(T::mount(None), id, "".to_string())
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
        _event: JoinEvent,
        values: &T,
    ) -> LiveViewSocketResult<(Self::State, Self::Reply), Self::Error> {
        let rendered = self
            .template_process
            .render_dynamic(values.clone())
            .expect("failed to render template");
        let state = RenderedJson::from(rendered.clone());
        let reply = json!({ "rendered": RenderedJson::from(rendered) });
        LiveViewSocketResult::Ok((state, reply))
    }

    fn handle_event(
        &self,
        state: &mut Self::State,
        _event: Event,
        values: &T,
    ) -> LiveViewSocketResult<Self::Reply, Self::Error> {
        let rendered = RenderedJson::from(
            self.template_process
                .render_dynamic(values.clone())
                .expect("failed to render template"),
        );

        let diff = state.diff(&rendered);
        *state = rendered;

        LiveViewSocketResult::Ok(json!({ "diff": diff }))
    }
}

#[derive(Debug, Error)]
pub enum LiveViewTeraError {
    #[error("event received before mount")]
    EventBeforeMount,
}

pub struct LiveViewTeraRenderer<T> {
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
        tera.register_function("csrf_token", csrf_token);
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

    #[terminate]
    fn terminate(self) {
        println!("Shutdown process");
    }

    #[handle_request]
    fn render_static(&self, value: T, id: String, session: String) -> Result<String, String> {
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

fn csrf_token(_: &HashMap<String, Value>) -> tera::Result<Value> {
    Ok(CsrfToken::get_or_init().masked.clone().into())
}
