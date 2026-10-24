//! Bounded async polling on event-driven platforms; input wakes the OS loop immediately.
use std::sync::{Arc, Condvar, Mutex};
struct State {
    interval: u64,
    stop: bool,
}
pub struct FrameWake {
    state: Arc<(Mutex<State>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Default for FrameWake {
    fn default() -> Self {
        let state = Arc::new((
            Mutex::new(State {
                interval: 16,
                stop: false,
            }),
            Condvar::new(),
        ));
        #[cfg(any(target_os = "windows", target_os = "android"))]
        let thread = {
            let state = state.clone();
            #[cfg(target_os = "windows")]
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetCurrentThreadId() -> u32;
            }
            #[cfg(target_os = "windows")]
            #[link(name = "user32")]
            unsafe extern "system" {
                fn PostThreadMessageW(thread: u32, message: u32, w: usize, l: isize) -> i32;
            }
            #[cfg(target_os = "windows")]
            let ui = unsafe { GetCurrentThreadId() };
            Some(std::thread::spawn(move || {
                let (lock, ready) = &*state;
                loop {
                    let Ok(current) = lock.lock() else { return };
                    if current.stop {
                        return;
                    }
                    let delay = if crate::platform::suspended() {
                        1000
                    } else {
                        current.interval
                    };
                    let Ok((current, _)) =
                        ready.wait_timeout(current, std::time::Duration::from_millis(delay))
                    else {
                        return;
                    };
                    if current.stop {
                        return;
                    }
                    drop(current);
                    if crate::platform::suspended() {
                        continue;
                    }
                    macroquad::miniquad::window::schedule_update();
                    // Miniquad's request channel alone cannot wake Win32 GetMessage.
                    #[cfg(target_os = "windows")]
                    unsafe {
                        PostThreadMessageW(ui, 0, 0, 0);
                    }
                }
            }))
        };
        #[cfg(not(any(target_os = "windows", target_os = "android")))]
        let thread = None;
        Self { state, thread }
    }
}
impl FrameWake {
    pub fn active(&self, active: bool) {
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            let interval = if active { 16 } else { 250 };
            if state.interval != interval {
                state.interval = interval;
                ready.notify_one();
            }
        }
    }
}
impl Drop for FrameWake {
    fn drop(&mut self) {
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            state.stop = true;
            ready.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
