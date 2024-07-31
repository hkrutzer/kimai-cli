use chrono::NaiveTime;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub endpoint: String,
    pub token: String,
    #[serde(default = "default_start_time")]
    pub default_start_time: NaiveTime,
}

fn default_start_time() -> NaiveTime {
    NaiveTime::from_hms_opt(9, 0, 0).unwrap()
}

pub fn load_config() -> anyhow::Result<Config> {
    let config = Figment::new()
        .merge(Toml::file("kimai.toml"))
        .merge(Env::prefixed("KIMAI_"))
        .extract()?;
    Ok(config)
}
