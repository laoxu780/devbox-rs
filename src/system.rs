//! macOS system-level shell: global hotkey, window restore and dock reopen.
//!
//! GPUI does not expose a global hotkey API, so Alt+Space is registered with
//! the Carbon Event Manager (the same mechanism Wails uses on macOS). The
//! callback arrives on the main thread; work is handed back to GPUI through
//! its foreground executor to avoid re-entering an active `App` borrow.

use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, AsyncApp, Bounds, TitlebarOptions, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};

use crate::ui::app::{DevBoxApp, Services};

/// State shared by the entry point, the dock-reopen handler and the hotkey.
pub struct AppRuntime {
    pub services: Services,
    pub window: RefCell<Option<WindowHandle<DevBoxApp>>>,
}

impl AppRuntime {
    pub fn new(services: Services) -> Self {
        Self {
            services,
            window: RefCell::new(None),
        }
    }
}

fn window_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1240.0), px(820.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("DevBox".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

/// Open (or re-open) the main window. `open_palette` is used by the global
/// shortcut so the command palette is ready as soon as the window appears.
pub fn open_main_window(
    cx: &mut App,
    runtime: &Rc<AppRuntime>,
    open_palette: bool,
) -> WindowHandle<DevBoxApp> {
    let services = runtime.services.clone();
    let options = window_options(cx);
    let handle = cx
        .open_window(options, move |_, cx| {
            let view = cx.new(|cx| DevBoxApp::new(services.clone(), cx));
            if open_palette {
                view.update(cx, |app, cx| app.open_palette(cx));
            }
            view
        })
        .expect("无法创建 DevBox 窗口");

    let id = handle.window_id();
    let weak_runtime = Rc::downgrade(runtime);
    cx.on_window_closed(move |_| {
        if let Some(runtime) = weak_runtime.upgrade() {
            if runtime.window.borrow().as_ref().map(|h| h.window_id()) == Some(id) {
                *runtime.window.borrow_mut() = None;
            }
        }
    })
    .detach();

    *runtime.window.borrow_mut() = Some(handle);
    handle
}

/// Bring DevBox back: focus the existing window, or create a new one after the
/// user closed it. When `toggle_palette` is set, the palette follows the
/// window state (open when closed, dismiss when already open).
pub fn restore_window(cx: &mut App, runtime: &Rc<AppRuntime>, toggle_palette: bool) {
    cx.activate(true);
    if let Some(handle) = *runtime.window.borrow() {
        let result = handle.update(cx, |app, window, cx| {
            window.activate_window();
            if toggle_palette {
                app.toggle_palette(cx);
            }
        });
        if result.is_ok() {
            return;
        }
    }
    let handle = open_main_window(cx, runtime, toggle_palette);
    handle.update(cx, |_, window, _| window.activate_window()).ok();
}

// ---------------------------------------------------------------------------
// Carbon event hotkey (Alt+Space)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

type EventTargetRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventRef = *mut c_void;
type OSStatus = i32;

const EVENT_CLASS_KEYBOARD: u32 = 0x6B626420; // 'kbd '
const EVENT_HOTKEY_PRESSED: u32 = 5;
const EVENT_PARAM_DIRECT_OBJECT: u32 = 0x2D2D2D2D; // '----'
const TYPE_EVENT_HOTKEY_ID: u32 = 0x686B6964; // 'hkid'
const OPTION_KEY_MODIFIER: u32 = 1 << 11;
const SPACE_KEY_CODE: u32 = 49;
const HOTKEY_SIGNATURE: u32 = 0x44425831; // 'DBX1'
const HOTKEY_ID: u32 = 1;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        hotkey_id: EventHotKeyID,
        target: EventTargetRef,
        out_ref: *mut *mut c_void,
    ) -> OSStatus;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus,
        num_types: u32,
        types: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut *mut c_void,
    ) -> OSStatus;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: u32,
        actual_size: *mut u32,
        buffer: *mut c_void,
    ) -> OSStatus;
}

struct HotkeyState {
    runtime: Rc<AppRuntime>,
    async_app: AsyncApp,
}

extern "C" fn on_hotkey(
    _call_ref: EventHandlerCallRef,
    event: EventRef,
    user_data: *mut c_void,
) -> OSStatus {
    unsafe {
        let mut hotkey = EventHotKeyID {
            signature: 0,
            id: 0,
        };
        let mut size: u32 = 0;
        let status = GetEventParameter(
            event,
            EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOTKEY_ID,
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyID>() as u32,
            &mut size,
            &mut hotkey as *mut _ as *mut c_void,
        );
        if status == 0 && hotkey.signature == HOTKEY_SIGNATURE && hotkey.id == HOTKEY_ID {
            let state = &*(user_data as *const HotkeyState);
            let runtime = state.runtime.clone();
            let async_app = state.async_app.clone();
            let executor_app = async_app.clone();
            async_app
                .foreground_executor()
                .spawn(async move {
                    let _ = executor_app.update(|cx| restore_window(cx, &runtime, true));
                })
                .detach();
        }
    }
    0
}

/// Register Alt+Space. Returns false when macOS rejects the registration
/// (for example another app already owns the combination).
pub fn install_alt_space_hotkey(runtime: Rc<AppRuntime>, async_app: AsyncApp) -> bool {
    let state = Box::new(HotkeyState {
        runtime,
        async_app,
    });
    let user_data = Box::into_raw(state) as *mut c_void;
    unsafe {
        let target = GetApplicationEventTarget();
        let types = [EventTypeSpec {
            event_class: EVENT_CLASS_KEYBOARD,
            event_kind: EVENT_HOTKEY_PRESSED,
        }];
        let mut handler: *mut c_void = std::ptr::null_mut();
        if InstallEventHandler(
            target,
            on_hotkey,
            types.len() as u32,
            types.as_ptr(),
            user_data,
            &mut handler,
        ) != 0
        {
            drop(Box::from_raw(user_data as *mut HotkeyState));
            return false;
        }
        let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
        RegisterEventHotKey(
            SPACE_KEY_CODE,
            OPTION_KEY_MODIFIER,
            EventHotKeyID {
                signature: HOTKEY_SIGNATURE,
                id: HOTKEY_ID,
            },
            target,
            &mut hotkey_ref,
        ) == 0
    }
}
