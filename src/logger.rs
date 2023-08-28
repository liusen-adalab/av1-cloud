use std::io::IsTerminal;

use ::time::{macros::format_description, UtcOffset};
use anyhow::{Context, Result};
use serde::Deserialize;
use tracing_subscriber::{
    fmt::{self, time::OffsetTime},
    prelude::__tracing_subscriber_SubscriberExt,
    EnvFilter, Layer,
};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub level: String,
}

static ADDITION_DERECTIVE: &[&str] = &["hyper=warn", "neli=warn", "actix_server::worker=warn"];

pub fn init(config: &Config) -> Result<()> {
    let std_out = {
        let mut filter = EnvFilter::from_default_env().add_directive(config.level.parse()?);
        for d in ADDITION_DERECTIVE {
            filter = filter.add_directive(d.parse().unwrap());
        }
        let offset = UtcOffset::current_local_offset().context("should get local offset!")?;
        let timer = OffsetTime::new(
            offset,
            format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]"
            ),
        );
        fmt::Layer::new()
            .with_ansi(std::io::stdout().is_terminal())
            .with_timer(timer)
            .with_target(true)
            .with_writer(std::io::stdout)
            .with_file(false)
            .with_filter(filter)
    };

    let collector_std = tracing_subscriber::registry().with(std_out);
    tracing::subscriber::set_global_default(collector_std).expect("failed to init logger");
    Ok(())
}
