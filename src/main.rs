extern crate clap;
extern crate sled;
use clap::Parser;
extern crate anyhow;
use anyhow::Result;
extern crate tracing;
use tracing::{debug, error, info};
extern crate axum;
use axum::{
    routing::{get, post},
    Router,
};
extern crate serde;
extern crate tokio;
extern crate tracing_subscriber;
use chrono::{Duration, NaiveTime};
use sploosh::{
    handlers::{alltimers, new_daily_form, new_timer, view_timer},
    util::{run_timer, AppState, GpioManager},
};
use std::{path::PathBuf, sync::Arc};

#[derive(Parser, Debug)]
struct Args {
    /// Absolute or relative path to the database directory
    #[arg(short, long)]
    db: PathBuf,
}

#[tokio::main]
async fn run(args: Args) -> Result<()> {
    let db_arc = Arc::new(sled::open(&args.db)?);
    let (man, gpio_tx) = GpioManager::new()?;
    let _ = man.run()?;
    info!("Opened database at {:?}", &args.db.display());
    let state = AppState {
        db: db_arc.clone(),
        gpio_tx: gpio_tx.clone(),
    };
    // build our application with a route
    let app = Router::new() // `GET /` goes to `root`
        .route("/", get(sploosh::handlers::root))
        // `POST /new_timer
        .route("/new_submit", post(new_daily_form))
        .route("/new_timer", get(new_timer))
        .route("/all_timers", get(alltimers))
        .route("/timer/:id", get(view_timer))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// wrapper to trace the async runtime
fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt::init();
    debug!("Args: {:?}", args);
    run(args)
        .map_err(|e| {
            error!("{}", e);
        })
        .unwrap();
    Ok(())
}
