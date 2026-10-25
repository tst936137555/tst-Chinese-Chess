use std::path::PathBuf;

#[cfg(target_os = "android")]
fn activity<'local>(
    env: &jni::JNIEnv<'local>,
) -> jni::objects::AutoLocal<'local, jni::objects::JObject<'local>> {
    // NewLocalRef runs under miniquad's replacement lock. AutoLocal releases it
    // before the AttachGuard detaches, including all early-return/error paths.
    env.auto_local(unsafe {
        jni::objects::JObject::from_raw(
            macroquad::miniquad::native::android::activity_ref(env.get_raw().cast()).cast(),
        )
    })
}

pub fn touch_delivery() -> [u64; 2] {
    #[cfg(target_os = "android")]
    return macroquad::miniquad::native::android::touch_delivery();
    #[cfg(not(target_os = "android"))]
    [0; 2]
}

#[cfg(target_os = "android")]
pub fn background_app() -> Result<(), String> {
    let vm = JVM.get().ok_or("Android 运行环境尚未就绪")?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = activity(&env);
    env.call_method(&activity, "xiangqiBackground", "()V", &[])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn observation_path() -> Option<PathBuf> {
    #[cfg(target_os = "ios")]
    if std::env::args().any(|a| a == "--native-ui-test") {
        return data_dir().ok().map(|p| p.join("interaction.json"));
    }
    #[cfg(target_os = "android")]
    if let Some(path) = OBSERVATION.get() {
        return Some(path.clone());
    }
    std::env::var_os("XIANGQI_OBSERVE_PATH").map(PathBuf::from)
}
pub fn keep_awake(enabled: bool) {
    #[cfg(target_os = "android")]
    {
        use jni::objects::JValue;
        static PREVIOUS: std::sync::Mutex<Option<(u64, bool)>> = std::sync::Mutex::new(None);
        let Ok(mut previous) = PREVIOUS.lock() else {
            return;
        };
        let current = (ui_generation(), enabled);
        if *previous == Some(current) {
            return;
        }
        let Some(vm) = JVM.get() else { return };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let activity = activity(&env);
        if env
            .call_method(
                &activity,
                "xiangqiKeepAwake",
                "(Z)V",
                &[JValue::Bool(u8::from(enabled))],
            )
            .is_ok()
        {
            *previous = Some(current);
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = enabled;
}
pub fn ui_generation() -> u64 {
    #[cfg(target_os = "android")]
    return macroquad::miniquad::native::android::activity_generation();
    #[cfg(not(target_os = "android"))]
    0
}
static VIEW_ORIGIN: [std::sync::atomic::AtomicI32; 2] =
    [const { std::sync::atomic::AtomicI32::new(0) }; 2];
pub fn view_origin() -> [i32; 2] {
    std::array::from_fn(|i| VIEW_ORIGIN[i].load(std::sync::atomic::Ordering::Acquire))
}
#[cfg(target_os = "android")]
pub fn publish_accessibility(json: &str) {
    use jni::objects::JValue;
    let Some(vm) = JVM.get() else {
        return;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return;
    };
    let activity = activity(&env);
    if let Ok(text) = env.new_string(json) {
        let _ = env.call_method(
            &activity,
            "xiangqiAccessibility",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&text)],
        );
        let _ = env.delete_local_ref(text);
    }
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiAccessibilityActivate(
    _: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    id: i32,
) {
    crate::accessibility::activate(id as u32);
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiViewOrigin(
    _: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    x: i32,
    y: i32,
) {
    for (i, v) in [x, y].into_iter().enumerate() {
        VIEW_ORIGIN[i].store(v, std::sync::atomic::Ordering::Release);
    }
}
#[cfg(target_os = "android")]
static OBSERVATION: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiObserve(
    mut env: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    path: jni::objects::JString<'_>,
) {
    if let Ok(value) = env.get_string(&path) {
        let value: String = value.into();
        let _ = OBSERVATION.set(value.into());
    }
}

pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XIANGQI_DATA_DIR") {
        return Ok(path.into());
    }
    #[cfg(target_os = "android")]
    if let Some(path) = OBSERVATION.get() {
        return Ok(path.parent().ok_or("测试目录无效")?.join("data"));
    }
    #[cfg(target_os = "android")]
    {
        use jni::objects::JString;
        static DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        if let Some(path) = DATA_DIR.get() {
            return Ok(path.clone());
        }
        // Called on miniquad's persistent rendering thread; the Activity is a global JNI ref.
        let path = {
            // The persistence/search threads are native workers. AttachGuard detaches
            // only threads attached here; an already attached rendering thread stays attached.
            let vm = JVM.get().ok_or("Android JVM 尚未初始化")?;
            let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
            let activity = activity(&env);
            let dir = env
                .call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])
                .and_then(|v| v.l())
                .map_err(|e| e.to_string())?;
            let string = JString::from(
                env.call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
                    .and_then(|v| v.l())
                    .map_err(|e| e.to_string())?,
            );
            let path: String = env.get_string(&string).map_err(|e| e.to_string())?.into();
            env.delete_local_ref(string).map_err(|e| e.to_string())?;
            env.delete_local_ref(dir).map_err(|e| e.to_string())?;
            path
        };
        let path = PathBuf::from(path);
        let _ = DATA_DIR.set(path.clone());
        return Ok(path);
    }
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or("APPDATA 未设置")?;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let base = PathBuf::from(std::env::var_os("HOME").ok_or("HOME 未设置")?)
        .join("Library/Application Support");
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
        .ok_or("无法定位数据目录")?;
    #[cfg(not(target_os = "android"))]
    {
        let path = base.join("tst-xiangqi-rust");
        #[cfg(target_os = "ios")]
        if std::env::args().any(|a| a == "--native-ui-test") {
            return Ok(path.join("ui-acceptance"));
        }
        Ok(path)
    }
}

pub fn engine_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("PIKAFISH_PATH") {
        return std::fs::canonicalize(path).map_err(|e| e.to_string());
    }
    let name = if cfg!(target_os = "windows") {
        "pikafish.exe"
    } else {
        "pikafish"
    };
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        paths.push(dir.join("assets/engine").join(name));
        paths.push(dir.join(name));
    }
    if let Ok(dir) = std::env::current_dir() {
        paths.push(dir.join("assets/engine").join(name));
        paths.push(dir.join(name));
    }
    paths
        .into_iter()
        .find(|p| p.is_file())
        .ok_or("未找到皮卡鱼引擎。请运行下载脚本，或设置 PIKAFISH_PATH。".into())
        .and_then(|p| p.canonicalize().map_err(|e| e.to_string()))
}
pub fn nnue_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("PIKAFISH_NNUE") {
        return std::fs::canonicalize(path).map_err(|e| e.to_string());
    }
    let mut paths = vec![];
    if let Ok(dir) = data_dir() {
        paths.push(dir.join("pikafish.nnue"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        paths.push(dir.join("assets/engine/pikafish.nnue"));
        paths.push(dir.join("pikafish.nnue"));
    }
    if let Ok(dir) = std::env::current_dir() {
        paths.push(dir.join("assets/engine/pikafish.nnue"));
    }
    paths
        .into_iter()
        .find(|p| p.is_file())
        .ok_or("未找到 NNUE 权重，请运行 tool/fetch_engine.ps1。".into())
        .and_then(|p| p.canonicalize().map_err(|e| e.to_string()))
}
#[cfg(target_os = "android")]
static JVM: std::sync::OnceLock<jni::JavaVM> = std::sync::OnceLock::new();
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiInit(
    env: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JVM.set(vm);
    }
}

// Native lifecycle callbacks are safe on platform threads and do not touch rendering state.
static SUSPENDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PAUSE_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static BACK_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn back_serial() -> u64 {
    BACK_SERIAL.load(std::sync::atomic::Ordering::Acquire)
}
pub fn suspended() -> bool {
    SUSPENDED.load(std::sync::atomic::Ordering::Acquire)
}
pub fn pause_serial() -> u64 {
    PAUSE_SERIAL.load(std::sync::atomic::Ordering::Acquire)
}
pub fn set_suspended(paused: bool) {
    if paused && !SUSPENDED.swap(paused, std::sync::atomic::Ordering::AcqRel) {
        PAUSE_SERIAL.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    } else if !paused {
        SUSPENDED.store(false, std::sync::atomic::Ordering::Release);
    }
}
pub fn take_back() -> bool {
    BACK.swap(false, std::sync::atomic::Ordering::AcqRel)
}

/// Synchronize a real mobile text editor without converting composition into backspaces.
pub fn name_editor(text: &str, start: usize, end: usize, rect: [f32; 4], visible: bool) {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        type State = (String, usize, usize, [u32; 4], bool, u64);
        static PREVIOUS: std::sync::Mutex<Option<State>> = std::sync::Mutex::new(None);
        let next = (
            text.to_owned(),
            start,
            end,
            rect.map(f32::to_bits),
            visible,
            ui_generation(),
        );
        if let Ok(mut previous) = PREVIOUS.lock() {
            if previous.as_ref() == Some(&next) {
                return;
            }
            *previous = Some(next);
        }
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let _ = (text, start, end, rect, visible);
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn xq_name_editor(json: *const std::ffi::c_char);
        }
        let json =
            serde_json::json!({"text":text,"start":start,"end":end,"rect":rect,"visible":visible})
                .to_string();
        if let Ok(json) = std::ffi::CString::new(json) {
            unsafe {
                xq_name_editor(json.as_ptr());
            }
        }
    }
    #[cfg(target_os = "android")]
    {
        use jni::objects::JValue;
        let Some(vm) = JVM.get() else {
            return;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let activity = activity(&env);
        if let Ok(value) = env.new_string(text) {
            let _ = env.call_method(
                &activity,
                "xiangqiNameState",
                "(Ljava/lang/String;IIZ)V",
                &[
                    JValue::Object(&value),
                    JValue::Int(start as i32),
                    JValue::Int(end as i32),
                    JValue::Bool(u8::from(visible)),
                ],
            );
            let _ = env.delete_local_ref(value);
        }
    }
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiNameEdit(
    mut env: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    text: jni::objects::JString<'_>,
    start: i32,
    end: i32,
) {
    if let Ok(text) = env.get_string(&text) {
        crate::text_input::native_edit(text.into(), start.max(0) as usize, end.max(0) as usize);
    }
}
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn xq_set_suspended(paused: bool) {
    set_suspended(paused);
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiPaused(
    _: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    paused: jni::sys::jboolean,
) {
    set_suspended(paused != 0);
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiBack(
    _: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
) {
    BACK_SERIAL.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    BACK.store(true, std::sync::atomic::Ordering::Release);
}

/// Install only bytes matching the release manifest, using atomic replacement.
pub fn install_nnue(
    destination: &std::path::Path,
    bytes: &[u8],
    expected: &str,
) -> Result<(), String> {
    install_nnue_reader(destination, &mut std::io::Cursor::new(bytes), expected)
}

pub fn install_nnue_reader(
    destination: &std::path::Path,
    reader: &mut impl std::io::Read,
    expected: &str,
) -> Result<(), String> {
    install_nnue_chunks(destination, expected, |chunk| {
        reader.read(chunk).map_err(|e| e.to_string())
    })
}

fn install_nnue_chunks(
    destination: &std::path::Path,
    expected: &str,
    mut read: impl FnMut(&mut [u8]) -> Result<usize, String>,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let parent = destination.parent().ok_or("权重目录无效")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if n > chunk.len() {
            return Err("权重读取长度无效".into());
        }
        hash.update(&chunk[..n]);
        tmp.write_all(&chunk[..n]).map_err(|e| e.to_string())?;
    }
    if format!("{:x}", hash.finalize()) != expected.to_ascii_lowercase() {
        return Err("包内 NNUE 校验失败".into());
    }
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    tmp.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn prepare_nnue(destination: &std::path::Path, expected: &str) -> Result<(), String> {
    if nnue_matches(destination, expected) {
        return Ok(());
    }
    #[cfg(target_os = "ios")]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut file = std::fs::File::open(
            exe.parent()
                .ok_or("应用资源目录无效")?
                .join("engine/pikafish.nnue"),
        )
        .map_err(|e| e.to_string())?;
        install_nnue_reader(destination, &mut file, expected)
    }
    #[cfg(target_os = "android")]
    {
        use jni::objects::{JObject, JValue};
        let vm = JVM.get().ok_or("Android JVM 尚未初始化")?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let activity = activity(&env);
        let assets = env
            .call_method(
                &activity,
                "getAssets",
                "()Landroid/content/res/AssetManager;",
                &[],
            )
            .and_then(|v| v.l())
            .map_err(|e| e.to_string())?;
        let name = env
            .new_string("engine/pikafish.nnue")
            .map_err(|e| e.to_string())?;
        let stream = env
            .call_method(
                &assets,
                "open",
                "(Ljava/lang/String;)Ljava/io/InputStream;",
                &[JValue::Object(&name)],
            )
            .and_then(|v| v.l())
            .map_err(|e| e.to_string())?;
        let buffer = env.new_byte_array(65536).map_err(|e| e.to_string())?;
        let mut signed = vec![0i8; 65536];
        let result = install_nnue_chunks(destination, expected, |chunk| {
            let n = env
                .call_method(&stream, "read", "([B)I", &[JValue::Object(&buffer)])
                .and_then(|v| v.i())
                .map_err(|e| e.to_string())?;
            if n < 0 {
                return Ok(0);
            }
            if n == 0 {
                return Err("权重资源读取没有进展".into());
            }
            let n = n as usize;
            env.get_byte_array_region(&buffer, 0, &mut signed[..n])
                .map_err(|e| e.to_string())?;
            for (a, b) in chunk.iter_mut().zip(&signed[..n]) {
                *a = *b as u8;
            }
            Ok(n)
        });
        let _ = env.call_method(&stream, "close", "()V", &[]);
        for object in [JObject::from(buffer), JObject::from(name), stream, assets] {
            let _ = env.delete_local_ref(object);
        }
        result
    }
}
pub fn nnue_matches(path: &std::path::Path, expected: &str) -> bool {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        match f.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hash.update(&buffer[..n]),
            Err(_) => return false,
        }
    }
    format!("{:x}", hash.finalize()) == expected.to_ascii_lowercase()
}
#[cfg(target_os = "android")]
static SMOKE_OUTPUT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
pub fn smoke_output() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "android")]
    {
        SMOKE_OUTPUT.get().cloned()
    }
    #[cfg(not(target_os = "android"))]
    {
        std::env::var_os("XIANGQI_SMOKE_OUTPUT").map(PathBuf::from)
    }
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_quad_1native_QuadNative_xiangqiSmoke(
    mut env: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    path: jni::objects::JString<'_>,
) {
    if let Ok(value) = env.get_string(&path) {
        let value: String = value.into();
        let root = PathBuf::from(value);
        // Record entry before EGL/audio initialization so emulator failures retain a stage.
        let _ = std::fs::create_dir_all(&root);
        let _ = std::fs::write(root.join("startup.txt"), "native-start");
        let _ = SMOKE_OUTPUT.set(root);
    }
}
static SAFE_INSETS: [std::sync::atomic::AtomicU32; 4] =
    [const { std::sync::atomic::AtomicU32::new(0) }; 4];
pub fn safe_insets() -> [f32; 4] {
    std::array::from_fn(|i| {
        f32::from_bits(SAFE_INSETS[i].load(std::sync::atomic::Ordering::Acquire))
    })
}
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn xq_set_safe_insets(top: f32, right: f32, bottom: f32, left: f32) {
    for (i, v) in [top, right, bottom, left].into_iter().enumerate() {
        SAFE_INSETS[i].store(v.max(0.).to_bits(), std::sync::atomic::Ordering::Release);
    }
}
