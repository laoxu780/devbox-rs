//! Non-macOS shell: window creation and dock-style restore without the
//! Carbon global hotkey. Keeps the entry point platform-independent.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, AsyncApp, Bounds, TitlebarOptions, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};

use crate::ui::app::{DevBoxApp, Services};

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

pub fn open_main_window(
    cx: &mut App,
    runtime: &Rc<AppRuntime>,
    _open_palette: bool,
) -> WindowHandle<DevBoxApp> {
    let services = runtime.services.clone();
    let handle = cx
        .open_window(window_options(cx), move |_, cx| {
            cx.new(move |cx| DevBoxApp::new(services.clone(), cx))
        })
        .expect("无法创建 DevBox 窗口");

    let id = handle.window_id();
    let weak = Rc::downgrade(runtime);
    cx.on_window_closed(move |_| {
        if let Some(runtime) = weak.upgrade()
            && runtime.window.borrow().as_ref().map(|h| h.window_id()) == Some(id)
        {
            *runtime.window.borrow_mut() = None;
        }
    })
    .detach();

    *runtime.window.borrow_mut() = Some(handle);
    handle
}

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

pub fn install_alt_space_hotkey(_runtime: Rc<AppRuntime>, _async_app: AsyncApp) -> bool {
    false
}
