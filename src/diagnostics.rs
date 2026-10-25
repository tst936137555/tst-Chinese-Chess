//! Bounded diagnostic history, independent of UI rendering and external services.
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};
static LOG: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
static LOCAL: OnceLock<std::sync::mpsc::SyncSender<String>> = OnceLock::new();
static CAPTURED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Keep the first failure of a process locally, off the rendering thread. Rotate
/// one previous report; never capture game libraries, names or environment values.
pub fn enable_local(directory: &std::path::Path) {
    if LOCAL.get().is_some() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::sync_channel::<String>(1);
    if LOCAL.set(tx).is_err() {
        return;
    }
    let directory = directory.to_owned();
    std::thread::spawn(move || {
        if let Ok(report) = rx.recv() {
            if std::fs::create_dir_all(&directory).is_err() {
                return;
            }
            let path = directory.join("last-failure.txt");
            if let Ok(previous) = std::fs::read(&path)
                && previous.len() <= 256 * 1024
            {
                let _ = crate::storage::Storage::atomic_write(
                    &directory.join("previous-failure.txt"),
                    &previous,
                );
            }
            let _ = crate::storage::Storage::atomic_write(&path, report.as_bytes());
        }
    });
}
pub fn redact(message: &str) -> String {
    let mut result: String = message.chars().take(2048).collect();
    for name in [
        "USERPROFILE",
        "HOME",
        "XIANGQI_DATA_DIR",
        "PIKAFISH_PATH",
        "PIKAFISH_NNUE",
    ] {
        if let Some(value) = std::env::var_os(name).filter(|s| !s.is_empty()) {
            let path = value.to_string_lossy();
            result = result
                .replace(path.as_ref(), "<local>")
                .replace(&path.replace('\\', "/"), "<local>");
        }
    }
    result
}
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
    if let Some(tx) = LOCAL.get()
        && !CAPTURED.swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        // Store structural evidence and timings, not arbitrary engine stderr or
        // user-entered text from the in-memory log.
        let report = format!(
            "version={} platform={} arch={} time={}\nstage={} kind={kind:?}\nmessage={}\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            crate::game::now_ms(),
            stage,
            redact(message)
        );
        let _ = tx.try_send(report);
    }
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
