pub mod csrf;
pub mod handler;
pub mod manager;
pub mod maud;
pub mod rendered;
pub mod socket;

use rendered::Rendered;
use serde::{Deserialize, Serialize};
use socket::Event;
use submillisecond::http::Uri;
use submillisecond::response::Response;
use submillisecond::RequestContext;
use thiserror::Error;

/// A live view.
pub trait LiveView: Sized {
    type Events: EventList<Self>;

    fn render(&self) -> Rendered;

    fn mount(uri: Uri) -> Self;

    fn styles() -> &'static [&'static str] {
        &[]
    }

    fn scripts() -> &'static [&'static str] {
        &[]
    }

    fn not_found(_req: RequestContext) -> Response {
        submillisecond::defaults::err_404()
    }
}

/// Live view event handler.
pub trait LiveViewEvent<E> {
    fn handle(state: &mut Self, event: E, event_type: String);
}

/// Event list is a trait to handle an incoming live view event.
pub trait EventList<T> {
    fn handle_event(state: &mut T, event: Event) -> Result<bool, DeserializeEventError>;
}

impl<T> EventList<T> for () {
    fn handle_event(_state: &mut T, _event: Event) -> Result<bool, DeserializeEventError> {
        Ok(false)
    }
}

#[derive(Debug, Error)]
pub enum DeserializeEventError {
    #[error(transparent)]
    Form(#[from] serde_qs::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

macro_rules! impl_event_list {
    ($( $t: ident ),*) => {
        impl<T, $( $t ),*> EventList<T> for ($( $t, )*)
        where
            $(
                T: LiveViewEvent<$t>,
                $t: for<'de> Deserialize<'de>,
            )*
        {
            fn handle_event(state: &mut T, event: Event) -> Result<bool, DeserializeEventError> {
                $(
                    if std::any::type_name::<$t>() == event.name {
                        let value: $t = if event.ty == "form" {
                            match event.value.as_str() {
                                Some(value) => serde_qs::from_str(value)?,
                                None => {
                                    return Err(DeserializeEventError::Form(serde_qs::Error::Custom(
                                        "expected value to be string in form event".to_string(),
                                    )));
                                }
                            }
                        } else {
                            serde_json::from_value(event.value)?
                        };
                        T::handle(state, value, event.ty);
                        return Ok(true);
                    }
                )*

                Ok(false)
            }
        }
    };
}

impl_event_list!(A);
impl_event_list!(A, B);
impl_event_list!(A, B, C);
impl_event_list!(A, B, C, D);
impl_event_list!(A, B, C, D, E);
impl_event_list!(A, B, C, D, E, F);
impl_event_list!(A, B, C, D, E, F, G);
impl_event_list!(A, B, C, D, E, F, G, H);
impl_event_list!(A, B, C, D, E, F, G, H, I);
impl_event_list!(A, B, C, D, E, F, G, H, I, J);
impl_event_list!(A, B, C, D, E, F, G, H, I, J, K);
impl_event_list!(A, B, C, D, E, F, G, H, I, J, K, L);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckboxValue {
    #[serde(rename = "on")]
    Checked,
    #[serde(rename = "off")]
    Unchecked,
}

impl CheckboxValue {
    pub fn is_checked(&self) -> bool {
        match self {
            CheckboxValue::Checked => true,
            CheckboxValue::Unchecked => false,
        }
    }
}

impl Default for CheckboxValue {
    fn default() -> Self {
        CheckboxValue::Unchecked
    }
}
