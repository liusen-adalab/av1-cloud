use anyhow::Result;
use serde::Deserialize;
use tracing_subscriber::{
    fmt::{self, format::Writer, time::FormatTime},
    prelude::__tracing_subscriber_SubscriberExt,
    EnvFilter, Layer,
};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub level: String,
}

struct LocalTimer;
impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f")
        )
    }
}

static ADDITION_DERECTIVE: &[&str] = &["hyper=warn", "neli=warn", "actix_server::worker=warn"];

pub fn init(config: &Config) -> Result<()> {
    let std_out = {
        let mut filter = EnvFilter::from_default_env().add_directive(config.level.parse()?);
        for d in ADDITION_DERECTIVE {
            filter = filter.add_directive(d.parse().unwrap());
        }
        fmt::Layer::new()
            .with_timer(LocalTimer)
            .with_target(true)
            .with_writer(std::io::stdout)
            .with_file(false)
            .with_filter(filter)
    };

    let collector_std = tracing_subscriber::registry().with(std_out);
    tracing::subscriber::set_global_default(collector_std).expect("failed to init logger");
    Ok(())
}
