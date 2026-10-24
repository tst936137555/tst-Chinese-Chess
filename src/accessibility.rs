//! Shared semantic controls and OS actions, independent of drawing and hit testing.
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: u32,
    pub label: String,
    pub rect: [f32; 4],
    pub actionable: bool,
    #[serde(default)]
    pub control: bool,
    #[serde(default)]
    pub disabled: bool,
    pub value: Option<String>,
}
impl Element {
    pub fn new(label: String, rect: [f32; 4], actionable: bool) -> Self {
        let identity = Self::label_identity(&label).to_owned();
        Self::with_identity(&identity, label, rect, actionable)
    }
    pub fn label_identity(label: &str) -> &str {
        if label.starts_with('第') {
            label.split('，').next().unwrap_or(label)
        } else {
            label
                .split('：')
                .next()
                .unwrap_or(label)
                .trim_end_matches("，已选中")
        }
    }
    /// Layout and translated/stateful labels are attributes, never node identities.
    pub fn with_identity(identity: &str, label: String, rect: [f32; 4], actionable: bool) -> Self {
        let mut id = 2166136261u32;
        for byte in identity.bytes() {
            id = (id ^ u32::from(byte)).wrapping_mul(16777619);
        }
        Self {
            id: (id & 0x3fff_ffff).max(16),
            label,
            rect,
            actionable,
            control: actionable,
            disabled: false,
            value: None,
        }
    }
}
#[derive(Clone, Default, PartialEq, Serialize)]
struct Snapshot {
    summary: String,
    elements: Vec<Element>,
    dpi: f32,
    focus: Option<u32>,
}
static SNAPSHOT: OnceLock<Mutex<Snapshot>> = OnceLock::new();
#[cfg(any(target_os = "windows", target_os = "macos"))]
static DESKTOP_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlAction {
    Click(u32),
    Focus(u32),
}
static ACTIONS: OnceLock<Mutex<std::collections::VecDeque<ControlAction>>> = OnceLock::new();
pub fn activate(id: u32) {
    enqueue_action(ControlAction::Click(id));
}
pub fn focus(id: u32) {
    enqueue_action(ControlAction::Focus(id));
}
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn desktop_action(action: accesskit::Action, id: u32) {
    match action {
        accesskit::Action::Click => activate(id),
        accesskit::Action::Focus => focus(id),
        _ => {}
    }
}
fn enqueue_action(action: ControlAction) {
    if let Ok(mut actions) = ACTIONS.get_or_init(Default::default).lock()
        && actions.len() < 32
    {
        actions.push_back(action);
    }
}
pub fn take_action() -> Option<ControlAction> {
    ACTIONS
        .get_or_init(Default::default)
        .lock()
        .ok()?
        .pop_front()
}
pub fn publish(summary: String, elements: Vec<Element>, dpi: f32, focus: Option<u32>) {
    let snapshot = Snapshot {
        summary,
        elements,
        dpi,
        focus,
    };
    let changed = if let Ok(mut previous) = SNAPSHOT.get_or_init(Default::default).lock() {
        if *previous == snapshot {
            false
        } else {
            *previous = snapshot.clone();
            true
        }
    } else {
        false
    };
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    if changed || !DESKTOP_READY.load(std::sync::atomic::Ordering::Acquire) {
        desktop::update();
    }
    if changed {
        #[cfg(target_os = "android")]
        crate::platform::publish_accessibility(
            &serde_json::to_string(&snapshot).unwrap_or_default(),
        );
        #[cfg(target_os = "ios")]
        {
            unsafe extern "C" {
                fn xq_accessibility_update(json: *const std::ffi::c_char);
            }
            if let Ok(json) =
                std::ffi::CString::new(serde_json::to_string(&snapshot).unwrap_or_default())
            {
                unsafe {
                    xq_accessibility_update(json.as_ptr());
                }
            }
        }
    }
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn xq_accessibility_activate(id: u32) {
    activate(id);
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod desktop {
    use super::*;
    use accesskit::{
        Action, ActionHandler, ActionRequest, ActivationHandler, Live, Node, NodeId, Rect, Role,
        TreeId, TreeInfo, TreeUpdate,
    };
    struct Handler;
    impl ActionHandler for Handler {
        fn do_action(&mut self, request: ActionRequest) {
            desktop_action(request.action, request.target_node.0 as u32);
            if request.action == Action::SetValue
                && let Some(accesskit::ActionData::Value(value)) = request.data
                && SNAPSHOT
                    .get_or_init(Default::default)
                    .lock()
                    .is_ok_and(|s| {
                        s.elements
                            .iter()
                            .any(|e| u64::from(e.id) == request.target_node.0 && e.value.is_some())
                    })
            {
                let cursor = value.chars().count();
                crate::text_input::native_edit(value.into(), cursor, cursor);
            }
        }
    }
    impl ActivationHandler for Handler {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            Some(tree())
        }
    }
    fn tree() -> TreeUpdate {
        let snapshot = SNAPSHOT
            .get_or_init(Default::default)
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let mut root = Node::new(Role::Window);
        root.set_label("中国象棋");
        let mut status = Node::new(Role::Status);
        status.set_label(snapshot.summary);
        status.set_live(Live::Polite);
        let mut children = vec![NodeId(2)];
        let mut nodes = vec![(NodeId(2), status)];
        for element in snapshot.elements {
            let id = NodeId(u64::from(element.id));
            let mut node = Node::new(if element.value.is_some() {
                Role::TextInput
            } else if element.control {
                Role::Button
            } else {
                Role::Label
            });
            node.set_label(element.label);
            if element.disabled {
                node.set_disabled();
            }
            if element.control && !element.disabled {
                node.add_action(Action::Focus);
            }
            if let Some(value) = element.value {
                node.set_value(value);
                node.add_action(Action::SetValue);
                node.add_action(Action::Focus);
            }
            let [x, y, w, h] = element.rect;
            let scale = if cfg!(target_os = "windows") {
                f64::from(snapshot.dpi)
            } else {
                1.
            };
            node.set_bounds(Rect::new(
                f64::from(x) * scale,
                f64::from(y) * scale,
                f64::from(x + w) * scale,
                f64::from(y + h) * scale,
            ));
            if element.actionable {
                node.add_action(Action::Click);
            }
            children.push(id);
            nodes.push((id, node));
        }
        let focus = snapshot
            .focus
            .filter(|id| children.contains(&NodeId(u64::from(*id))))
            .map_or(NodeId(1), |id| NodeId(u64::from(id)));
        root.set_children(children);
        nodes.push((NodeId(1), root));
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(NodeId(1))),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    #[cfg(target_os = "windows")]
    mod windows {
        use super::*;
        use accesskit_windows::{Adapter, HWND, LPARAM, LRESULT, WPARAM};
        use std::{cell::RefCell, ffi::c_void};
        type Callback =
            unsafe extern "system" fn(*mut c_void, u32, usize, isize, usize, usize) -> isize;
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetActiveWindow() -> *mut c_void;
        }
        #[link(name = "comctl32")]
        unsafe extern "system" {
            fn SetWindowSubclass(
                hwnd: *mut c_void,
                callback: Option<Callback>,
                id: usize,
                data: usize,
            ) -> i32;
            fn DefSubclassProc(
                hwnd: *mut c_void,
                message: u32,
                wparam: usize,
                lparam: isize,
            ) -> isize;
        }
        thread_local! { static ADAPTER: RefCell<Option<Adapter>> = const { RefCell::new(None) }; }
        unsafe extern "system" fn callback(
            hwnd: *mut c_void,
            message: u32,
            wparam: usize,
            lparam: isize,
            _id: usize,
            _data: usize,
        ) -> isize {
            if message == 0x003d {
                let result = ADAPTER.with(|cell| {
                    let mut guard = cell.try_borrow_mut().ok()?;
                    let adapter = guard.as_mut()?;
                    adapter
                        .handle_wm_getobject(WPARAM(wparam), LPARAM(lparam), &mut Handler)
                        .map(|result| {
                            let result: LRESULT = result.into();
                            result.0
                        })
                });
                if let Some(result) = result {
                    return result;
                }
            }
            if message == 7 || message == 8 {
                let events = ADAPTER.with(|cell| {
                    cell.try_borrow_mut()
                        .ok()
                        .and_then(|mut a| a.as_mut()?.update_window_focus_state(message == 7))
                });
                if let Some(events) = events {
                    events.raise();
                }
            }
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        pub fn update() {
            let events = ADAPTER.with(|cell| {
                let mut guard = cell.borrow_mut();
                if guard.is_none() {
                    let hwnd = unsafe { GetActiveWindow() };
                    if hwnd.is_null() {
                        return None;
                    }
                    let adapter = Adapter::new(HWND(hwnd), true, Handler);
                    if unsafe { SetWindowSubclass(hwnd, Some(callback), 0x5851, 0) } == 0 {
                        return None;
                    }
                    *guard = Some(adapter);
                    DESKTOP_READY.store(true, std::sync::atomic::Ordering::Release);
                }
                guard.as_mut()?.update_if_active(tree)
            });
            if let Some(events) = events {
                events.raise();
            }
        }
    }
    #[cfg(target_os = "windows")]
    pub use windows::update;

    #[cfg(target_os = "macos")]
    pub fn update() {
        use std::cell::RefCell;
        thread_local! { static ADAPTER: RefCell<Option<accesskit_macos::SubclassingAdapter>> = const { RefCell::new(None) }; }
        let events = ADAPTER.with(|cell| {
            let mut adapter = cell.borrow_mut();
            if adapter.is_none() {
                let view = macroquad::miniquad::window::apple_view();
                *adapter = Some(unsafe {
                    accesskit_macos::SubclassingAdapter::new(view.cast(), Handler, Handler)
                });
                DESKTOP_READY.store(true, std::sync::atomic::Ordering::Release);
            }
            adapter.as_mut()?.update_if_active(tree)
        });
        if let Some(events) = events {
            events.raise();
        }
    }
}
