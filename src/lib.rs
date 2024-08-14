#![feature(async_closure)]
extern crate bytes;
extern crate chrono;
use chrono::NaiveTime;
extern crate tokio;
extern crate uuid;
pub use uuid::Uuid;
extern crate serde;
use serde::{Deserialize, Serialize};
extern crate serde_json;
extern crate thiserror;

use std::time::Duration;
pub mod handlers;
use handlers::NewDaily;
pub mod util;
use util::{naive_now, Error};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IntervalTimer {
    id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    settings: IntervalSettings,
}

impl IntervalTimer {
    pub fn get_id(&self) -> Uuid {
        self.id
    }
    pub fn new(
        name: Option<String>,
        description: Option<String>,
        settings: IntervalSettings,
    ) -> IntervalTimer {
        let id = Uuid::new_v4();
        IntervalTimer {
            id,
            name,
            description,
            settings,
        }
    }

    pub fn once_daily(
        name: Option<String>,
        description: Option<String>,
        duration_on: Duration,
        start_time: NaiveTime,
    ) -> Result<IntervalTimer, Error> {
        let id = Uuid::new_v4();
        let settings = IntervalSettings::once_daily(duration_on, start_time)?;
        Ok(IntervalTimer {
            id,
            name,
            description,
            settings,
        })
    }

    pub fn daily_now(
        name: Option<String>,
        description: Option<String>,
        duration_on: Duration,
    ) -> Result<IntervalTimer, Error> {
        let id = Uuid::new_v4();
        let settings = IntervalSettings::daily_now(duration_on)?;
        Ok(IntervalTimer {
            id,
            name,
            description,
            settings,
        })
    }

    pub fn from_newdaily(n: NewDaily) -> Result<Self, Error> {
        let id = Uuid::new_v4();
        let name = Some(n.name.to_owned());
        let description = n.description.to_owned();
        let settings = IntervalSettings::from_newdaily(n)?;
        Ok(IntervalTimer {
            id,
            name,
            description,
            settings,
        })
    }

    /// Serialize the struct into a JSON string
    pub fn to_json_string(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(|e| util::Error::Json(e))
    }
    /// Serialize the struct to a JSON Vec<u8>
    pub fn to_json_vec(&self) -> Result<Vec<u8>, Error> {
        serde_json::to_vec(self).map_err(|e| util::Error::Json(e))
    }
    /// Deserialize a struct from bytes of JSON text
    pub fn from_json_slice(slice: impl AsRef<[u8]>) -> Result<Self, Error> {
        serde_json::from_slice(slice.as_ref()).map_err(|e| util::Error::Json(e))
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IntervalSettings {
    duration_on: Duration,
    duration_off: Duration,
    start_time: Option<NaiveTime>,
}

impl IntervalSettings {
    pub fn new(
        duration_on: Duration,
        duration_off: Duration,
        start_time: Option<NaiveTime>,
    ) -> IntervalSettings {
        IntervalSettings {
            duration_on,
            duration_off,
            start_time,
        }
    }

    pub fn once_daily(
        duration_on: Duration,
        start_time: NaiveTime,
    ) -> Result<IntervalSettings, Error> {
        if duration_on.is_zero() {
            Err(Error::InvalidDuration)
        } else {
            let duration_off = Duration::from_secs(60 * 60 * 24) - duration_on; // 24h-duration_on
            Ok(IntervalSettings {
                duration_on,
                duration_off,
                start_time: Some(start_time),
            })
        }
    }
    pub fn daily_now(duration_on: Duration) -> Result<IntervalSettings, Error> {
        IntervalSettings::once_daily(duration_on, naive_now())
    }

    pub fn from_newdaily(n: NewDaily) -> Result<IntervalSettings, Error> {
        let duration_on = Duration::from_secs(n.duration_on.into());
        let start_time = NaiveTime::parse_from_str(n.start_time.as_ref(), "%H:%M")
            .map_err(|e| Error::TimeParsing(e))?;
        IntervalSettings::once_daily(duration_on, start_time)
    }
}
