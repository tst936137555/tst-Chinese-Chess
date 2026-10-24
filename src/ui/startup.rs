use super::*;
pub(super) async fn initialize() -> (App, Option<smoke::Smoke>, Option<profile::Profile>) {
    let startup = std::time::Instant::now();
    let smoke = smoke::Smoke::from_args();
    let profile = profile::Profile::from_args();
    if let Some(s) = &smoke {
        s.boot("font-load");
    }
    let font = crate::typography::UiFont::load(include_bytes!(
        "../../assets/fonts/XqKai-Medium-subset.ttf"
    ))
    .expect("内置字体损坏");
    crate::diagnostics::measure("font_load_ms", startup.elapsed().as_secs_f64() * 1000.);
    let loader = loading::LoadingScreen::new(
        smoke
            .as_ref()
            .map(|s| s.state_path().with_file_name("startup.png")),
    );
    loader.show(&font, 0.04).await;
    let surface_job = drawing::Surfaces::prepare();
    if let Some(s) = &smoke {
        s.boot("render-target-start");
    }
    crate::diagnostics::measure("first_frame_ms", startup.elapsed().as_secs_f64() * 1000.);
    let scene_started = std::time::Instant::now();
    let scene = crate::rendering::AdaptiveScene::new();
    crate::diagnostics::measure(
        "scene_init_ms",
        scene_started.elapsed().as_secs_f64() * 1000.,
    );
    if let Some(s) = &smoke {
        s.boot("render-target-ready");
    }
    let (_, canvas_height) = layout::canvas_size(screen_width(), screen_height(), false);
    font.set_pixel_density(scene.target.texture.height() / canvas_height);
    let mut font_precache_ms = 0.;
    for (index, (text, size)) in copy::FONT_GROUPS.iter().enumerate() {
        // Help/about and long licenses are prepared when opened, after the first interaction.
        if index < 2 {
            continue;
        }
        if let Some(s) = &smoke {
            s.boot(&format!("font-group-{index}"));
        }
        let warm_started = std::time::Instant::now();
        font.prepare(text, *size);
        font_precache_ms += warm_started.elapsed().as_secs_f64() * 1000.;
        loader
            .show(
                &font,
                0.12 + 0.5 * (index + 1) as f32 / copy::FONT_GROUPS.len() as f32,
            )
            .await;
    }
    if let Some(s) = &smoke {
        s.boot("audio-start");
    }
    let audio = crate::audio::AudioBank::load().await;
    loader.show(&font, 0.68).await;
    let (mut path, mut notice) = match crate::platform::data_dir() {
        Ok(d) => (d.join("state.json"), String::new()),
        Err(e) => (std::path::PathBuf::from("state.json"), e),
    };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    if let Some(parent) = path.parent() {
        let dest = parent.join("pikafish.nnue");
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../tool/engine_manifest.json"))
                .expect("asset manifest");
        let expected = manifest["pikafish"]["artifacts"]["nnue"]["sha256"]
            .as_str()
            .expect("NNUE hash")
            .to_owned();
        let (done, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let result = crate::platform::prepare_nnue(&dest, &expected);
            crate::diagnostics::measure("nnue_prepare_ms", start.elapsed().as_secs_f64() * 1000.);
            let _ = done.send(result);
        });
        loop {
            match result.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(e)) => {
                    notice = format!("权重安装失败：{e}");
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    notice = "权重准备线程已退出".into();
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => loader.show(&font, 0.72).await,
            }
        }
    }
    let license_started = std::time::Instant::now();
    let mut license_texts = vec![
        include_str!("../../LICENSE").into(),
        String::new(),
        include_str!("../../assets/fonts/OFL.txt").into(),
    ];
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        license_texts[1] = load_string("licenses/NNUE-License.md")
            .await
            .unwrap_or_else(|e| format!("许可证加载失败：{e}"));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let mut paths = vec![std::path::PathBuf::from("assets/licenses/NNUE-License.md")];
        if let Ok(exe) = std::env::current_exe()
            && let Some(p) = exe.parent()
        {
            paths.push(p.join("assets/licenses/NNUE-License.md"));
        }
        license_texts[1] = paths
            .into_iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(|| "请先运行引擎资产下载脚本获取 NNUE 许可证。".into());
    }
    crate::diagnostics::measure(
        "license_load_ms",
        license_started.elapsed().as_secs_f64() * 1000.,
    );
    loader.show(&font, 0.92).await;
    crate::diagnostics::measure("font_precache_ms", font_precache_ms);
    if let Some(s) = &smoke {
        path = s.state_path();
    }
    if let Some(profile) = &profile {
        path = profile.state_path();
    }
    if let Some(s) = &smoke {
        s.boot("storage");
    }
    let store = Storage::new(path);
    let args: Vec<_> = std::env::args().collect();
    let import = args
        .iter()
        .position(|s| s == "--import")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);
    let (mut data, storage_failed, storage_notice) =
        bootstrap::read_storage(store.clone(), import, &loader, &font).await;
    if !storage_notice.is_empty() {
        notice = storage_notice;
    }
    let images = loop {
        match surface_job.try_recv() {
            Ok(images) => break images,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("内置纹理准备失败"),
            Err(std::sync::mpsc::TryRecvError::Empty) => loader.show(&font, 0.96).await,
        }
    };
    let surfaces = drawing::Surfaces::from_images(images);
    data.settings.level = data.settings.level.min(4);
    let settings = data.settings.clone();
    let mut app = App {
        font,
        surfaces,
        scene,
        audio,
        screen: Screen::Home,
        game: Game::default(),
        preferences: settings.clone(),
        settings,
        review_game: false,
        io: Worker::new(store),
        saves: crate::session::SaveState::default(),
        close: Default::default(),
        focus: None,
        focus_index: 0,
        focus_count: 0,
        keyboard_active: true,
        keyboard_nav: false,
        name_keyboard: false,
        rename_return: None,
        board_cursor: None,
        observed_controls: Vec::new(),
        observed_at: 0.,
        data,
        engine: Engine::new(),
        pending: None,
        ready_move: None,
        selected: None,
        hints: vec![],
        notice,
        engine_failed: false,
        review: None,
        navigation: Vec::new(),
        width: 640.,
        height: 900.,
        viewport: Rect::new(0., 0., screen_width(), screen_height()),
        click: None,
        modal: storage_failed.then_some(Modal::StorageRecovery),
        scroll: 0.,
        drag: None,
        animation: None,
        ended_at: None,
        end_ready: false,
        license_texts,
        modal_scroll: 0.,
        modal_scroll_grab: None,
        move_scroll: 0.,
        dragged: false,
        move_follow: true,
        move_layout_key: (String::new(), u64::MAX),
        move_labels: Vec::new(),
        move_offsets: Vec::new(),
        move_total: 0.,
        archive_order: Vec::new(),
        archive_dirty: true,
        archive_favorites: false,
        legal_key: String::new(),
        legal_cache: Vec::new(),
        text_cache: std::collections::HashMap::new(),
        recovery: crate::session::Recovery::default(),
        pause_serial: crate::platform::pause_serial(),
        piece_cache: std::cell::RefCell::new(std::collections::VecDeque::new()),
        board_surface_cache: std::cell::RefCell::new(None),
        accessible: Vec::new(),
        accessibility_active: true,
        accessibility_focus: None,
        semantic_scope: String::new(),
        semantic_counts: Default::default(),
        focus_request: None,
        redraw: true,
    };
    (app.width, app.height) = layout::canvas_size(app.viewport.w, app.viewport.h, false);
    if let Some(s) = &smoke {
        s.boot("board-cache");
    }
    app.warm_board();
    loader.show(&app.font, 1.).await;
    drop(loader);
    if let Some(s) = &smoke {
        s.boot("interactive");
    }
    crate::diagnostics::measure("interactive_ms", startup.elapsed().as_secs_f64() * 1000.);
    (app, smoke, profile)
}
