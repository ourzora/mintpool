# Developer docs

Documentation for working on mintpool its self.

## Adding metrics

We using `tracing` for both logging and metrics via the `tracing-opentelemetry` crate. See
docs [here](https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/struct.MetricsLayer.html).

This supports `monotonic_counter`, `counter`, and `histogram` metrics. You use them via `tracing`
but setting
a field on the span.

```rust
tracing::info!(histogram.metric_name = 0.9);
tracing::info!(monotonic_counter.number = 1);
tracing::info!(counter.example3 = 1, other_fields="will be included in tags", "Be careful about cardinality");
```

**Don't put log lines in the same trace statements as metrics, especially if theres a format
variable in the log line,
they will blow up you metrics cardinality and cost a lot of money if you use DataDog**

Metrics are also controlled by the `RUST_LOG` environment variable. This makes it easy to have debug
metrics, just use `tracing::debug!(histogram.debug_thing = 42.1)`.

In the mintpool default binary metrics are exported via Prometheus on `/metrics`.