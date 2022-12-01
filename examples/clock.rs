use std::time::Duration;

use chrono::Utc;
use lunatic::{Mailbox, MailboxResult, Process};
use serde::{Deserialize, Serialize};
use submillisecond::{router, static_router, Application};
use submillisecond_live_view::prelude::*;

fn main() -> std::io::Result<()> {
    Application::new(router! {
        GET "/" => Clock::handler()
        "/static" => static_router!("./static")
    })
    .serve("127.0.0.1:3000")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Clock {
    socket: Option<Socket>,
    tick_frequency: u64,
    ticker: Option<Process<u64>>,
    time: String,
    timezone: chrono_tz::Tz,
}

impl LiveView for Clock {
    type Events = (Tick, ChangeTimezone, ChangeTickFrequency);

    fn mount(_uri: Uri, socket: Option<Socket>) -> Self {
        let ticker = if let Some(socket) = socket.clone() {
            let ticker = Process::spawn_link(socket, |mut socket, mailbox: Mailbox<u64>| {
                let mut update_frequency = 500;
                loop {
                    match mailbox.receive_timeout(Duration::from_millis(update_frequency)) {
                        MailboxResult::Message(ms) => {
                            update_frequency = ms;
                        }
                        MailboxResult::TimedOut => {
                            socket.send_event(Tick {}).unwrap();
                        }
                        err => panic!("{err:?}"),
                    }
                }
            });
            // TODO: Use this code when <https://github.com/lunatic-solutions/lunatic-rs/pull/88> is merged and published.
            // let ticker = spawn_link!(|socket, mailbox: Mailbox<u64>| {});
            Some(ticker)
        } else {
            None
        };

        Clock {
            socket,
            tick_frequency: 500,
            ticker,
            time: Utc::now()
                .with_timezone(&chrono_tz::UTC)
                .format("%A, %H:%M:%S%.3f")
                .to_string(),
            timezone: chrono_tz::UTC,
        }
    }

    fn render(&self) -> Rendered<Self> {
        let tzs = chrono_tz::TZ_VARIANTS.iter();

        html! {
            h2 { (self.time) }
            form {
                select name="timezone" @change=(ChangeTimezone) {
                    @for tz in tzs {
                        @let selected = if tz == &self.timezone { Some("selected") } else { None };
                        option
                            value=(tz.name())
                            selected=[selected]
                        {
                            (tz.name())
                        }
                    }
                }
                br {}
                input
                    name="tick_frequency"
                    type="range"
                    min="100" max="1000"
                    value=(self.tick_frequency)
                    phx-throttle="500"
                    @change=(ChangeTickFrequency);
                br {}
                span { (format!("{}ms", self.tick_frequency)) }
            }
        }
    }

    fn head() -> Head {
        Head::defaults()
            .with_title("LiveView Clock")
            .with_style(Style::Link("/static/clock.css"))
    }
}

#[derive(Serialize, Deserialize)]
struct Tick {}

impl LiveViewEvent<Tick> for Clock {
    fn handle(state: &mut Self, _event: Tick) {
        state.time = Utc::now()
            .with_timezone(&state.timezone)
            .format("%A, %H:%M:%S%.3f")
            .to_string();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ChangeTimezone {
    timezone: String,
}

impl LiveViewEvent<ChangeTimezone> for Clock {
    fn handle(state: &mut Self, ChangeTimezone { timezone }: ChangeTimezone) {
        state.timezone = timezone.parse().unwrap();
        state.socket.as_mut().unwrap().spawn_send_event(Tick {});
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ChangeTickFrequency {
    tick_frequency: u64,
}

impl LiveViewEvent<ChangeTickFrequency> for Clock {
    fn handle(state: &mut Self, ChangeTickFrequency { tick_frequency }: ChangeTickFrequency) {
        state.tick_frequency = tick_frequency;
        state.ticker.as_ref().unwrap().send(tick_frequency);
    }
}
