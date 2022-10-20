use tracing::{error, info};

pub fn init_open_telemetry(pkg_name: &str, pkg_version: &str) {
    use opentelemetry::sdk::Resource;
    use opentelemetry::KeyValue;
    use std::time::Duration;

    match std::env::var("OTEL_EXPORTER") {
        Ok(exporter) if (exporter == "otlp") || (exporter == "otlp_metric") => {
            let resource_attributes = [
                KeyValue::new("service.name", pkg_name.to_owned()),
                KeyValue::new("service.version", pkg_version.to_owned()),
            ];
            let resource = Resource::new(resource_attributes);

            match opentelemetry_otlp::new_pipeline()
                .metrics(
                    opentelemetry::sdk::metrics::selectors::simple::inexpensive(),
                    opentelemetry::sdk::export::metrics::aggregation::cumulative_temporality_selector(),
                    opentelemetry::runtime::AsyncStd,
                )
                .with_exporter(opentelemetry_otlp::new_exporter().tonic())
                .with_period(Duration::from_secs(1))
                .with_timeout(Duration::from_secs(10))
                .with_resource(resource)
                .build() {
                Ok(_) => info!("initialized otlp metrics exporter"),
                Err(_) => error!("unable to initialize otlp metrics exporter")
            }
        }
        _ => (),
    }
}
