//! Bounded diagnostic history, independent of UI rendering and external services.
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};
static LOG: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
pub fn record(stage: &str, message: &str) {
    if let Ok(mut lines) = LOG.get_or_init(|| Mutex::new(VecDeque::new())).lock() {
        if lines.len() >= 200 {
            lines.pop_front();
        }
        let safe: String = message.chars().take(2048).collect();
        lines.push_back(format!(
            "{} [{stage}] {}",
            crate::game::now_ms(),
            safe.trim()
        ));
    }
}
pub fn snapshot() -> String {
    let log = LOG
        .get_or_init(|| Mutex::new(VecDeque::new()))
        .lock()
        .map(|l| l.iter().cloned().collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    let metrics = METRICS
        .get_or_init(|| Mutex::new(std::collections::BTreeMap::new()))
        .lock()
        .map(|m| {
            m.iter()
                .map(|(key, metric)| {
                    let mut sorted: Vec<_> = metric.recent.iter().copied().collect();
                    sorted.sort_by(f64::total_cmp);
                    format!(
                        "{key}: recent_samples={} p50={:.3} p95={:.3} max={:.3}; lifetime_samples={} mean={:.3} max={:.3}",
                        sorted.len(),
                        sorted[sorted.len() / 2],
                        sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)],
                        sorted[sorted.len() - 1], metric.samples, metric.total / metric.samples as f64, metric.max
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    format!("{log}\nPerformance (last 240 samples per metric):\n{metrics}")
}
#[derive(Default)]
struct Metric {
    recent: VecDeque<f64>,
    samples: u64,
    total: f64,
    max: f64,
}
type Metrics = std::collections::BTreeMap<String, Metric>;
static METRICS: OnceLock<Mutex<Metrics>> = OnceLock::new();
pub fn measure(name: &str, value: f64) {
    if !value.is_finite() {
        return;
    }
    if let Ok(mut metrics) = METRICS.get_or_init(|| Mutex::new(Metrics::new())).lock() {
        let metric = metrics.entry(name.into()).or_default();
        if metric.recent.len() >= 240 {
            metric.recent.pop_front();
        }
        metric.recent.push_back(value);
        metric.samples = metric.samples.saturating_add(1);
        metric.total += value;
        metric.max = metric.max.max(value);
    }
}
pub fn failure(kind: FailureKind, stage: &str, message: &str) {
    record(stage, &format!("{kind:?}: {message}"));
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Assets,
    Timeout,
    Cancelled,
    Protocol,
    Storage,
    Conflict,
    Other,
    RestartRequired,
}
