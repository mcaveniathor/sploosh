use crate::IntervalTimer;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, Local, NaiveTime};
use gpio::{
    sysfs::{SysFsGpioInput, SysFsGpioOutput},
    GpioOut,
};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};
use tracing::{debug, error, info, warn};

pub struct DailyTimer {
    pub time: NaiveTime,
    pub msg: GpioOutMessage,
    pub duration: Duration,
    pub tx: mpsc::Sender<GpioMessage>,
}

impl DailyTimer {
    pub fn new(
        time: NaiveTime,
        msg: GpioOutMessage,
        duration: Duration,
        tx: mpsc::Sender<GpioMessage>,
    ) -> DailyTimer {
        DailyTimer {
            time,
            msg,
            duration,
            tx,
        }
    }

    pub fn run(&self) -> JoinHandle<()> {
        let msg = self.msg;
        let off_msg = GpioOutMessage {
            output: self.msg.output,
            value: !self.msg.value,
        };
        let start_time = self.time;
        let stop_time = self.time + self.duration;
        let tx = self.tx.clone();
        let f = tokio::spawn(async move {
            info!("Spawned task to run new daily timer.");
            loop {
                info!("Waiting until {:?}", &start_time);
                TimeFuture::new(start_time).await;
                let _ = tx.send(msg.into()).await.map_err(|e| error!("{}", e));
                info!("Waiting until {:?}", &stop_time);
                TimeFuture::new(stop_time).await;
                let _ = tx.send(off_msg.into()).await.map_err(|e| error!("{}", e));
            }
        });
        f
    }
}

pub fn naive_now() -> NaiveTime {
    let dt = Local::now();
    dt.time()
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Duration cannot be zero")]
    InvalidDuration,
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Db(#[from] sled::Error),
    #[error("Failed to parse time from hh:mm format: {0}")]
    TimeParsing(#[from] chrono::ParseError),
    #[error("Other error: {0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Unknown error")]
    Unknown,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound(s) => (StatusCode::NOT_FOUND, s).into_response(),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response(),
        }
    }
}
#[derive(Debug, Copy, Clone)]
pub struct GpioOutMessage {
    pub output: u16,
    pub value: bool,
}

#[derive(Debug, Clone)]
pub enum GpioMessage {
    In(u16),
    Out(GpioOutMessage),
}

pub async fn run_timer(
    tx: mpsc::Sender<GpioMessage>,
    output: u16,
    value: bool,
    time: NaiveTime,
    duration: Duration,
) -> Result<(), Error> {
    let mut outmsg = GpioOutMessage { output, value };
    let _ = TimeFuture::new(time).await;
    tx.send(outmsg.clone().into())
        .await
        .map_err(|e| Error::Anyhow(e.into()))?;
    info!(
        "Sent message to set output {} to value {} for duration {}.",
        output, value, &duration
    );
    tokio::time::sleep(duration.to_std().map_err(|e| Error::Anyhow(e.into()))?).await;
    outmsg.value = !value;
    tx.send(outmsg.into())
        .await
        .map_err(|e| Error::Anyhow(e.into()))?;
    info!(
        "Sent message to set output {} back to value {}.",
        &output, !value
    );
    Ok(())
}

impl From<GpioOutMessage> for GpioMessage {
    fn from(other: GpioOutMessage) -> GpioMessage {
        GpioMessage::Out(other)
    }
}

#[derive(Debug)]
pub struct GpioManager {
    inputs: HashMap<u16, SysFsGpioInput>,
    outputs: HashMap<u16, SysFsGpioOutput>,
    rx: mpsc::Receiver<GpioMessage>,
}
impl GpioManager {
    pub fn new() -> Result<(GpioManager, mpsc::Sender<GpioMessage>), Error> {
        let (tx, rx) = mpsc::channel(32);
        let (inputs, outputs) = (HashMap::new(), HashMap::new());
        let man = GpioManager {
            inputs,
            outputs,
            rx,
        };
        Ok((man, tx))
    }
    pub fn run(self) -> Result<(), Error> {
        tokio::spawn(async move {
            let mut rx = self.rx;
            debug!("Spawned GPIO manager thread");
            while let Some(message) = rx.recv().await {
                info!("Received GPIO message: {:?}", &message);
                match message {
                    GpioMessage::In(num) => {
                        let pin = SysFsGpioInput::open(num).map_err(|e| {
                            error!("{}", e);
                        });
                        info!("Opened GPIO port {} for reading", &num);
                        warn!("GPIO in not yet implemented");
                    }
                    GpioMessage::Out(outmsg) => {
                        if let Ok(mut pin) = SysFsGpioOutput::open(outmsg.output) {
                            info!("Opened GPIO output {} for writing", &outmsg.output);
                            if let Ok(_) = pin.set_value(outmsg.value).map_err(|e| error!("{}", e))
                            {
                                info!("Write to pin {} successful.", &outmsg.output);
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

pub fn local_time() -> NaiveTime {
    let dt: DateTime<Local> = Local::now();
    dt.time()
}
pub fn time_until(target: NaiveTime) -> Duration {
    let now = local_time();
    let diff = target - now;
    if diff < Duration::zero() {
        // Target time is later in the day than now, add (negative) difference to 24h to get
        // positive time until target
        Duration::new(86400, 0).unwrap() + diff
    } else {
        diff
    }
}

pub struct TimeSharedState {
    completed: bool,
    waker: Option<Waker>,
}

/// A future that resolves at a given time
pub struct TimeFuture {
    shared_state: Arc<Mutex<TimeSharedState>>,
}
pub struct Daily {
    time: NaiveTime,
    duration: Duration,
}
impl Future for TimeFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl TimeFuture {
    /// Returns a future which will resolve at the next occurrence of `time` in the local timezone
    pub fn new(time: NaiveTime) -> Self {
        let shared_state = Arc::new(Mutex::new(TimeSharedState {
            completed: false,
            waker: None,
        }));
        let thread_shared_state = shared_state.clone();
        tokio::spawn(async move {
            let sleep_time = time_until(time);
            sleep(sleep_time.to_std().unwrap()).await;
            let mut shared_state = thread_shared_state.lock().unwrap();
            shared_state.completed = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake()
            }
        });
        TimeFuture { shared_state }
    }
}

pub struct Periodic {
    pulse_width: Duration,
    duty: f32,
    period: Duration,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<sled::Db>,
    pub gpio_tx: mpsc::Sender<GpioMessage>,
}
impl AppState {
    pub fn insert_interval_timer(
        &self,
        interval: &IntervalTimer,
    ) -> Result<Option<IntervalTimer>, Error> {
        let id = interval.get_id();
        let bytes = interval.to_json_vec()?;
        let prev = self.db.insert(id.as_bytes(), bytes)?;
        let prev = match prev {
            Some(ivec) => Some(IntervalTimer::from_json_slice(ivec.as_ref())?),
            _ => None,
        };
        Ok(prev)
    }

    pub fn get_interval_timer(&self, id: impl AsRef<[u8]>) -> Result<Option<IntervalTimer>, Error> {
        match self.db.get(id.as_ref())? {
            Some(value) => {
                let timer = IntervalTimer::from_json_slice(value.as_ref())?;
                Ok(Some(timer))
            }
            _ => Ok(None),
        }
    }

    pub fn get_all_interval_timers(&self) -> Result<Vec<IntervalTimer>, Error> {
        let result: Result<Vec<_>, _> = self
            .db
            .iter()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
            .iter()
            .map(|x| {
                let val = &x.1;
                IntervalTimer::from_json_slice(&val)
            })
            .collect();
        result
    }
}

markup::define! {
    Layout<Head: markup::Render, Main: markup::Render>(
        head: Head,
        main: Main,
    ) {
        @markup::doctype()
        html {
            head {
                @head
                style {
                    "nav{ background: #FFAAAA text-align: center }"
                    "body { background: #ECFFE6 }"
                    "columns { border-style: solid }"
                    "column { border-style: solid }"


                    @markup::raw(include_str!("../static/css/normalize.css"))
                    @markup::raw(include_str!("../static/css/skeleton.css"))
                    @markup::raw(
                        r#"
                        <link href="fonts.googleapis.com/css?family=Raleway:400,300,600" rel="stylesheet" type="text/css">
                        "#
                    )
                }
            }
            body {
                nav {
                    div .container {
                        div .row {
                            div .four.columns {
                                a[href = "/"] { "Home" }
                            }
                            div .four.columns {
                                a [href="/new_timer"] { "New Timer" }
                            }
                            div .four.columns {
                                a [href="/all_timers"] { "All Timers" }
                            }
                        }
                    }
                }
                main {
                    @main
                }
            }
        }
    }
}

pub mod skeleton {

    pub fn to_numcols(s: u8) -> String {
        match s {
            1 => "one column",
            2 => "two columns",
            3 => "three columns",
            4 => "four columns",
            5 => "five columns",
            6 => "six columns",
            7 => "seven columns",
            8 => "eight columns",
            9 => "nine columns",
            10 => "ten columns",
            11 => "eleven columns",
            _ => "twelve columns",
        }
        .to_string()
    }

    markup::define! {
        Columns<Contents: markup::Render>(
            number: u8,
            contents: Contents,
        ) {
            div .{to_numcols(*number)}
            {
                @contents
            }

        }
    }
}
