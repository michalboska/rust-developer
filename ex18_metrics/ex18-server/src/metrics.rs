use std::error::Error;
use std::sync::OnceLock;
use std::time::Duration;

use prometheus::{Encoder, Histogram, HistogramOpts, IntCounter, IntGauge, Registry, TextEncoder};

static INSTANCE: OnceLock<Metrics> = OnceLock::new();

pub struct Metrics {
    registry: Registry,
    encoder: TextEncoder,
    chat_messages_count: IntCounter,
    connected_users_count: IntGauge,
    sql_query_duration_histo: Histogram,
}

impl Metrics {
    pub fn instance() -> &'static Metrics {
        INSTANCE.get_or_init(|| {
            let registry = Registry::new();

            let instance = Metrics {
                registry,
                encoder: TextEncoder::new(),
                chat_messages_count: IntCounter::new(
                    "total_messages",
                    "Total messages sent in this server instance",
                )
                .unwrap(),
                connected_users_count: IntGauge::new(
                    "connected_users",
                    "Number of currently connected chat users",
                )
                .unwrap(),
                sql_query_duration_histo: Histogram::with_opts(HistogramOpts::new(
                    "sql_query_duration_ms",
                    "How many ms sql queries took",
                ))
                .unwrap(),
            };
            instance
                .registry
                .register(Box::new(instance.chat_messages_count.clone()))
                .unwrap();
            instance
                .registry
                .register(Box::new(instance.connected_users_count.clone()))
                .unwrap();
            instance
                .registry
                .register(Box::new(instance.sql_query_duration_histo.clone()))
                .unwrap();
            instance
        })
    }

    pub fn track_message_sent(&self) {
        self.chat_messages_count.inc()
    }

    pub fn track_user_connected(&self) {
        self.connected_users_count.inc()
    }

    pub fn track_user_disconnected(&self) {
        self.connected_users_count.dec()
    }

    pub fn track_sql(&self, dur: Duration) {
        self.sql_query_duration_histo
            .observe(dur.as_secs_f64() * 100f64)
    }

    pub fn export(&self) -> Result<String, Box<dyn Error>> {
        let mut buffer = Vec::new();
        let mut families = self.registry.gather();
        families.append(&mut prometheus::gather());
        self.encoder.encode(&families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}
