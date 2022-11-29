use std::path::Path;
use std::{fs, io};

use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use lunatic::abstract_process;
use lunatic::process::{ProcessRef, StartProcess};
use nipper::Document;
use once_cell::sync::Lazy;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sha2::Sha256;
use uuid::Uuid;

use crate::csrf::CsrfToken;
use crate::maud::{secret, Session};

static TEMPLATE_PROCESS_ID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());

#[cfg(all(debug_assertions, feature = "liveview_js"))]
const LIVEVIEW_JS: &str = include_str!("../dist/liveview-debug.js");

#[cfg(all(not(debug_assertions), feature = "liveview_js"))]
const LIVEVIEW_JS: &str = include_str!("../dist/liveview-release.js");

const HTML_SEPARATOR: &str = "<!-- SUBMILLISECOND_LIVE_VIEW_SEPARATOR -->";

pub struct TemplateProcess {
    html_parts: [String; 3],
}

#[abstract_process(visibility = pub(crate))]
impl TemplateProcess {
    #[init]
    fn init(_: ProcessRef<Self>, html: String) -> Self {
        let document = Document::from(&html.replace(0x0 as char, ""));
        document.select("head").append_html(format!(
            r#"{HTML_SEPARATOR}<script type="text/javascript">{LIVEVIEW_JS}</script>"#,
        ));
        document.select("body").append_html(HTML_SEPARATOR);
        let html_parts = document
            .html()
            .to_string()
            .splitn(3, HTML_SEPARATOR)
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        TemplateProcess { html_parts }
    }

    #[handle_request]
    fn render(&self, content: String) -> String {
        let mut html_parts = self.html_parts.clone();

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

        html_parts[0].push_str(&format!(
            r#"<meta name="csrf-token" content="{csrf_token}" />"#
        ));

        html_parts[1].push_str(&format!(
            r#"<div data-phx-main="true" data-phx-static="" data-phx-session={session_str} id={id}>{content}</div>"#
        ));

        html_parts.into_iter().collect()
    }

    pub fn lookup() -> Option<ProcessRef<Self>> {
        ProcessRef::lookup(&TEMPLATE_PROCESS_ID)
    }

    pub fn lookup_or_start<P: AsRef<Path>>(path: P) -> io::Result<ProcessRef<Self>> {
        let process = match ProcessRef::lookup(&TEMPLATE_PROCESS_ID) {
            Some(process) => process,
            None => {
                let template = fs::read_to_string(path)?;
                Self::start_link(template, Some(&TEMPLATE_PROCESS_ID))
            }
        };

        Ok(process)
    }
}
