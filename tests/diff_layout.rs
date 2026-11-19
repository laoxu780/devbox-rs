use gpui::{Modifiers, TestAppContext, point, px};

use devbox::proxy::ProxyService;
use devbox::ssh::SshService;
use devbox::ui::app::{DevBoxApp, Services, ToolId};

fn init_config_dir() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);
        // SAFETY: called once before any service or view is created.
        unsafe { std::env::set_var("DEVBOX_CONFIG_DIR", path) };
    });
}

#[gpui::test]
fn diff_fills_the_content_area(cx: &mut TestAppContext) {
    init_config_dir();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let services = Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), dir.path().join("api.json")),
        ssh: SshService::new_with_paths(
            runtime.handle().clone(),
            dir.path().join("profiles.json"),
            dir.path().join("forwards.json"),
        ),
        hosts: devbox::hosts::HostsService::with_path(dir.path().join("hosts.json")),
    };
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));

    for y in (60..760).step_by(6) {
        cx.simulate_click(point(px(100.0), px(y as f32)), Modifiers::none());
        if cx.update(|_, app| view.read(app).tool) == ToolId::Diff {
            break;
        }
    }
    assert_eq!(cx.update(|_, app| view.read(app).tool), ToolId::Diff);

    let bounds = cx.debug_bounds("diff_root").expect("diff root not rendered");
    println!(
        "WINDOW {:?} DIFF ROOT {:?}",
        cx.update(|window, _| window.bounds()),
        bounds
    );
    assert!(
        bounds.size.height > px(600.0),
        "diff area too short: {bounds:?}"
    );
    assert!(
        bounds.size.width > px(600.0),
        "diff area too narrow: {bounds:?}"
    );
    for selector in [
        "diff_input_left",
        "diff_input_right",
        "diff_result_left",
        "diff_result_right",
    ] {
        let area = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing four-pane area {selector}"));
        assert!(area.size.height > px(300.0), "{selector} too short: {area:?}");
        assert!(area.size.width > px(600.0), "{selector} too narrow: {area:?}");
    }
}

#[gpui::test]
fn json_fills_the_content_area(cx: &mut TestAppContext) {
    init_config_dir();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let services = Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), dir.path().join("api.json")),
        ssh: SshService::new_with_paths(
            runtime.handle().clone(),
            dir.path().join("profiles.json"),
            dir.path().join("forwards.json"),
        ),
        hosts: devbox::hosts::HostsService::with_path(dir.path().join("hosts.json")),
    };
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));

    for y in (60..760).step_by(6) {
        cx.simulate_click(point(px(100.0), px(y as f32)), Modifiers::none());
        if cx.update(|_, app| view.read(app).tool) == ToolId::Json {
            break;
        }
    }
    assert_eq!(cx.update(|_, app| view.read(app).tool), ToolId::Json);

    let root = cx.debug_bounds("json_root").expect("JSON root");
    assert!(root.size.height > px(600.0), "JSON root too short: {root:?}");
    for selector in ["json_input", "json_output"] {
        let area = cx.debug_bounds(selector).unwrap_or_else(|| panic!("missing {selector}"));
        assert!(area.size.height > px(500.0), "{selector} too short: {area:?}");
    }
}
