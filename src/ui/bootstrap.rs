use super::*;
pub(super) async fn read_storage(
    store: Storage,
    import: Option<std::path::PathBuf>,
    loader: &loading::LoadingScreen,
    font: &crate::typography::UiFont,
) -> (Data, bool, String) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let (mut data, mut failed, mut notice) = match store.load() {
            Ok(data) => (data, false, String::new()),
            Err(e) => {
                crate::diagnostics::failure(
                    crate::diagnostics::FailureKind::Storage,
                    "startup-storage",
                    &e,
                );
                (
                    Data {
                        settings: store.load_preferences(Settings::default()),
                        ..Data::default()
                    },
                    true,
                    e,
                )
            }
        };
        crate::diagnostics::measure("storage_load_ms", started.elapsed().as_secs_f64() * 1000.);
        if let Some(path) = import {
            let started = std::time::Instant::now();
            match store.import_legacy(&path) {
                Ok((imported, count)) => {
                    data = imported;
                    failed = false;
                    notice = format!("已导入 {count} 局旧棋谱");
                }
                Err(e) => notice = e,
            }
            crate::diagnostics::measure(
                "legacy_import_ms",
                started.elapsed().as_secs_f64() * 1000.,
            );
        }
        let _ = tx.send((data, failed, notice));
    });
    loop {
        match rx.try_recv() {
            Ok(value) => return value,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return (
                    Data::default(),
                    true,
                    "存档读取线程已退出，原文件已保留".into(),
                );
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => loader.show(font, 0.94).await,
        }
    }
}
