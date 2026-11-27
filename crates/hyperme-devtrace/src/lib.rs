//! Developer-only trace exports (specs/182-trace-store, R-15 to R-17).
//!
//! Compiled in only with the `trace-chrome` or `otel` feature, which a release build never
//! enables, and switched on only by an environment variable, never by the config file:
//!
//! - `HYPERME_TRACE_CHROME=<path>` writes a Perfetto-readable JSON trace of the session (R-15).
//! - `HYPERME_OTLP_ENDPOINT=<url>` exports spans over OTLP/HTTP to `<url>/v1/traces`, with
//!   headers from `HYPERME_OTLP_HEADERS` as `name=value,name=value` (R-16). The headers often
//!   hold a key, so they are never logged.
//!
//! Without either feature, [`Exports::from_env`] is always empty.

use tracing_subscriber::{Layer, Registry};

/// A layer the app adds under its own (see `hyperme_core::trace::subscriber_layers`).
pub type ExtraLayer = Box<dyn Layer<Registry> + Send + Sync>;

/// The exports switched on for this session, and what flushes them at exit.
#[derive(Default)]
pub struct Exports {
    layers: Vec<ExtraLayer>,
    #[cfg(feature = "trace-chrome")]
    chrome: Option<tracing_chrome::FlushGuard>,
    #[cfg(feature = "otel")]
    otel: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl std::fmt::Debug for Exports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the headers (R-16).
        f.debug_struct("Exports")
            .field("layers", &self.layers.len())
            .finish_non_exhaustive()
    }
}

impl Exports {
    /// The exports the environment asks for.
    pub fn from_env() -> Self {
        Self::from_vars(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
    }

    /// The exports that `var` asks for: [`Exports::from_env`] with another source, for tests.
    #[allow(unused_variables, unused_mut)]
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let mut exports = Self::default();
        #[cfg(feature = "trace-chrome")]
        if let Some(path) = var("HYPERME_TRACE_CHROME") {
            let (layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
                .file(path)
                .include_args(true)
                .build();
            exports.layers.push(Box::new(layer));
            exports.chrome = Some(guard);
        }
        #[cfg(feature = "otel")]
        if let Some(endpoint) = var("HYPERME_OTLP_ENDPOINT") {
            let headers = var("HYPERME_OTLP_HEADERS").unwrap_or_default();
            if let Some((layer, provider)) = otel::layer(&endpoint, &headers) {
                exports.layers.push(layer);
                exports.otel = Some(provider);
            }
        }
        exports
    }

    /// Nothing is exported.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// The layers to add to the subscriber; the exports keep what flushes them.
    pub fn take_layers(&mut self) -> Vec<ExtraLayer> {
        std::mem::take(&mut self.layers)
    }

    /// Writes out what is still buffered: the end of the Perfetto file, the last OTLP batch.
    /// Call once, at exit.
    pub fn shutdown(&mut self) {
        #[cfg(feature = "trace-chrome")]
        drop(self.chrome.take());
        #[cfg(feature = "otel")]
        if let Some(provider) = self.otel.take() {
            let _ = provider.shutdown();
        }
    }
}

#[cfg(feature = "otel")]
mod otel {
    use std::collections::HashMap;

    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
    use opentelemetry_sdk::Resource;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    use super::ExtraLayer;

    /// `name=value,name=value`, as OTEL_EXPORTER_OTLP_HEADERS.
    pub(crate) fn headers(text: &str) -> HashMap<String, String> {
        text.split(',')
            .filter_map(|pair| pair.split_once('='))
            .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
            .filter(|(name, _)| !name.is_empty())
            .collect()
    }

    pub(crate) fn layer(
        endpoint: &str,
        header_text: &str,
    ) -> Option<(ExtraLayer, SdkTracerProvider)> {
        let base = endpoint.trim_end_matches('/');
        let url = if base.ends_with("/v1/traces") {
            base.to_owned()
        } else {
            format!("{base}/v1/traces")
        };
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(url)
            .with_headers(headers(header_text))
            .build()
            .ok()?;
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(Resource::builder().with_service_name("hyperme").build())
            .build();
        let tracer = provider.tracer("hyperme");
        let layer = tracing_opentelemetry::layer().with_tracer(tracer);
        Some((Box::new(layer), provider))
    }
}
