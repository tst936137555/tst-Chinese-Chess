use std::path::PathBuf;

pub fn observation_path() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    if let Some(path) = OBSERVATION.get() {
        return Some(path.clone());
    }
    std::env::var_os("XIANGQI_OBSERVE_PATH").map(PathBuf::from)
}
static VIEW_ORIGIN: [std::sync::atomic::AtomicI32; 2] =
    [const { std::sync::atomic::AtomicI32::new(0) }; 2];
pub fn view_origin() -> [i32; 2] {
    std::array::from_fn(|i| VIEW_ORIGIN[i].load(std::sync::atomic::Ordering::Acquire))
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
        use jni::objects::{JObject, JString};
        use macroquad::miniquad::native::android;
        // Called on miniquad's persistent rendering thread; the Activity is a global JNI ref.
        let path = unsafe {
            // The persistence/search threads are native workers. AttachGuard detaches
            // only threads attached here; an already attached rendering thread stays attached.
            let vm = JVM.get().ok_or("Android JVM 尚未初始化")?;
            let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
            let activity = JObject::from_raw(android::ACTIVITY.cast());
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
        return Ok(PathBuf::from(path));
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
    Ok(base.join("tst-xiangqi-rust"))
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
    BACK.store(true, std::sync::atomic::Ordering::Release);
}

/// Install only bytes matching the release manifest, using atomic replacement.
pub fn install_nnue(
    destination: &std::path::Path,
    bytes: &[u8],
    expected: &str,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    if format!("{:x}", Sha256::digest(bytes)) != expected.to_ascii_lowercase() {
        return Err("包内 NNUE 校验失败".into());
    }
    let parent = destination.parent().ok_or("权重目录无效")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    tmp.write_all(bytes)
        .and_then(|()| tmp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    tmp.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
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
        None
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
        let _ = SMOKE_OUTPUT.set(value.into());
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
