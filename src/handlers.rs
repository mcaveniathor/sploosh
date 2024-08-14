use crate::{
    util::{naive_now, AppState, DailyTimer, GpioOutMessage, Layout},
    Error, IntervalTimer,
};
use axum::{
    extract::{Path, State},
    response::Redirect,
    Form,
};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

#[axum::debug_handler]
pub async fn new_daily_form(
    State(state): State<AppState>,
    Form(n): Form<NewDaily>,
) -> Result<Redirect, Error> {
    let timer = IntervalTimer::from_newdaily(n)?;
    let prev = state.insert_interval_timer(&timer)?;
    info!(
        "Inserted timer {:?} into the database. Previous value: {:?}",
        &timer, &prev
    );
    let timer = DailyTimer::new(
        timer.settings.start_time.unwrap_or(naive_now()),
        GpioOutMessage {
            output: 476,
            value: true,
        }
        .into(),
        Duration::from_std(timer.settings.duration_on).unwrap(),
        state.gpio_tx.clone(),
    );
    timer.run();

    Ok(Redirect::to("/"))
}

#[axum::debug_handler]
pub async fn update_daily_form(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Form(n): Form<NewDaily>,
) -> Result<Redirect, Error> {
    let mut timer = IntervalTimer::from_newdaily(n)?;
    timer.id = id;
    let prev = state.insert_interval_timer(&timer)?;
    info!(
        "Inserted timer {:?} into the database. Previous value: {:?}",
        &timer, &prev
    );
    Ok(Redirect::to("/"))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewDaily {
    /// The name of the new timer
    pub name: String,
    pub description: Option<String>,
    /// Duration in seconds
    pub duration_on: u32,
    /// Time of day to run, in %H:%M format
    pub start_time: String,
}

#[axum::debug_handler]
pub async fn new_timer() -> impl axum::response::IntoResponse {
    let template = Layout {
        head: markup::new! {
            title { "Home" }
        },
        main: markup::new! {
            div .container {
                div .row {
                    div .twelve.columns {
                        h1 { "New Daily Timer" }
                    }
                }
                form[action = "/new_submit", method = "post"] {
                    div .row {
                        div .six.columns {
                            label[for = "name"] { "Name" }
                            input[id = "name", name = "name", type = "text", required];
                            label[for = "Description"] { "Description" }
                            textarea[id = "description", name = "description", rows = 7] {}
                        }
                        div .six.columns {
                            label[for = "duration_on"] { "Duration (mins)" }
                            input[id = "duration_ob", name = "duration_on", type = "number", required];
                            label[for = "start_time"] { "Start Time" }
                            input[id = "start_time", name = "start_time", type = "time", required];
                            br {}
                            button[type = "submit"] { "Submit" }
                        }
                    }
                }
            }
        },
    };
    axum::response::Html(template.to_string())
}

#[axum::debug_handler]
pub async fn alltimers(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let all = state.get_all_interval_timers()?;
    let template = Layout {
        head: markup::new! {
            title { "All Timers" }
        },
        main: markup::new! {
            div .container {
                div .row {
                    div .twelve.columns {
                        h1 { "All Timers" }
                    }
                }
                table ."u-full-width" {
                    thead {
                        tr {
                            th {"Name"}
                            th {"Description"}
                            th {"Duration"}
                            th {"Start Time"}
                        }
                    }
                    tbody {
                        @for t in &all {
                            tr {
                                td {
                                    a [href=format!("/timer/{}", t.id)] { @t.name }
                                }
                                td { @t.description}
                                td { @format!("{:?}", t.settings.duration_on)}
                                td { @t.settings.start_time.unwrap_or_default().to_string()}
                            }
                        }
                    }
                }
            }
        },
    };
    Result::<_, Error>::Ok(axum::response::Html(template.to_string()))
}

#[axum::debug_handler]
pub async fn root(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let template = Layout {
        head: markup::new! {
            title { "Homepage" }
        },
        main: markup::new! {
            div .container {
                div .row {
                    div .twelve.columns {
                        h1 { "Home" }
                    }
                }

            }
        },
    };
    Result::<_, Error>::Ok(axum::response::Html(template.to_string()))
}

#[axum::debug_handler]
pub async fn view_timer(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if let Some(timer) = state.get_interval_timer(&id)? {
        let template = Layout {
            head: markup::new! {
                title { "Timer" }
            },
            main: markup::new! {
                div .container {
                    div .row {
                        div .twelve.columns {
                            h1 { @timer.name }
                            p { @timer.description}
                        }
                    }
                form[action = format!("/new_submit/{}",timer.id), method = "post"] {
                        div .row {
                            div .six.columns {
                                label[for = "name"] { "Name" }
                                input[id = "name", name = "name", type = "text", value = timer.name.clone(), required];
                                label[for = "Description"] { "Description" }
                                textarea[id = "description", name = "description", rows = 7, value = timer.description.clone() ] {}
                            }
                            div .six.columns {
                                label[for = "duration_on"] { "Duration (mins)" }
                                input[id = "duration_ob", name = "duration_on", type = "number", value = timer.settings.duration_on.as_secs(), required];
                                label[for = "start_time"] { "Start Time" }
                                input[id = "start_time", name = "start_time", type = "time", value = timer.settings.start_time.unwrap().format("%-I:%M %p").to_string(), required];
                                br {}
                                button[type = "submit"] { "Save" }
                            }
                        }
                    }
                }
            },
        };
        Result::<_, Error>::Ok(axum::response::Html(template.to_string()))
    } else {
        Err(Error::NotFound(format!("Timer with ID {}", &id)))
    }
}
