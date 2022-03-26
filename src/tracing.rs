use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

pub fn setup_tracing() {
    let app_name =
        concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"))
            .to_string();

    let formatting_layer =
        BunyanFormattingLayer::new(app_name, std::io::stdout);
    let json_subs = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with(JsonStorageLayer)
        .with(formatting_layer);

    let mut fmt_subs = tracing_subscriber::fmt().with_env_filter(
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info")),
    );

    if cfg!(feature = "tracing_noansi") {
        fmt_subs = fmt_subs.with_ansi(false)
    }

    let fmt_subs = fmt_subs.finish();

    if cfg!(feature = "tracing_json") {
        tracing::subscriber::set_global_default(json_subs)
            .expect("Couldn't set global tracing susbscriber");
    } else {
        tracing::subscriber::set_global_default(fmt_subs)
            .expect("Couldn't set global tracing susbscriber");
    }
}
