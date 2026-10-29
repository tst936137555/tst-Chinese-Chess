use super::*;
use std::{
    ffi::{CStr, CString, c_void},
    sync::{Mutex, OnceLock},
};
unsafe extern "C" {
    fn sf_prepare() -> i32;
    fn sf_start(line: extern "C" fn(*const u8), exit: extern "C" fn(i32)) -> i32;
    fn sf_send(line: *const u8) -> i32;
    fn sf_stop();
    fn sf_free(ptr: *mut c_void);
}
static OUTPUT: OnceLock<Mutex<Option<mpsc::Sender<String>>>> = OnceLock::new();
extern "C" fn on_line(p: *const u8) {
    if p.is_null() {
        return;
    }
    // The shim allocates a NUL-terminated line; sf_free is its matching allocator.
    let s = unsafe { CStr::from_ptr(p.cast()).to_string_lossy().into_owned() };
    unsafe {
        sf_free(p.cast_mut().cast());
    }
    if let Ok(guard) = OUTPUT.get_or_init(|| Mutex::new(None)).lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(s);
        }
    }
}
extern "C" fn on_exit(_code: i32) {
    if let Ok(mut guard) = OUTPUT.get_or_init(|| Mutex::new(None)).lock() {
        *guard = None;
    }
}
pub struct Ffi {
    rx: Receiver<String>,
}
impl Ffi {
    pub fn open() -> Result<Self, String> {
        // Drain the old exit callback before publishing the new receiver.
        if unsafe { sf_prepare() } != 0 {
            return Err("进程内引擎尚未停止，请保存后重启应用".into());
        }
        let (tx, rx) = mpsc::channel();
        *OUTPUT
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|e| e.to_string())? = Some(tx);
        let rc = unsafe { sf_start(on_line, on_exit) };
        if rc != 0 {
            if let Ok(mut guard) = OUTPUT.get_or_init(|| Mutex::new(None)).lock() {
                *guard = None;
            }
            return Err("进程内引擎未能启动，可能需要重启应用".into());
        }
        Ok(Self { rx })
    }
}
impl Transport for Ffi {
    fn send(&mut self, line: &str) -> Result<(), String> {
        let s = CString::new(line).map_err(|e| e.to_string())?;
        if unsafe { sf_send(s.as_ptr().cast()) } == 0 {
            Ok(())
        } else {
            Err("进程内引擎已停止".into())
        }
    }
    fn receive(&mut self, timeout: Duration) -> Result<Option<String>, String> {
        match self.rx.recv_timeout(timeout) {
            Ok(s) => Ok(Some(s)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err("进程内引擎已退出".into()),
        }
    }
}
impl Drop for Ffi {
    fn drop(&mut self) {
        unsafe {
            sf_stop();
        }
    }
}
