use std::io::Write;

use gpui::{Modifiers, TestAppContext, VisualTestContext, point};

use devbox::proxy::ProxyService;
use devbox::ssh::SshService;
use devbox::ui::app::{DevBoxApp, Services, ToolId};

fn init_config_dir() -> std::path::PathBuf {
    static INIT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);
        // SAFETY: called once before any service or view is created.
        unsafe { std::env::set_var("DEVBOX_CONFIG_DIR", path.clone()) };
        path
    })
    .clone()
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) {
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(value.to_string().as_bytes()).unwrap();
}

fn goto(view: &gpui::Entity<DevBoxApp>, cx: &mut VisualTestContext, tool: ToolId) {
    for y in (60..760).step_by(6) {
        cx.simulate_click(point(gpui::px(100.0), gpui::px(y as f32)), Modifiers::none());
        if cx.update(|_, app| view.read(app).tool) == tool {
            return;
        }
    }
    panic!("could not navigate to {tool:?}");
}

/// Regression test: the mode chips in the new-forward modal (local / SOCKS5 /
/// HTTP) each need a unique element id. When they shared one literal id, only
/// the first chip could be clicked, which made SOCKS5 and HTTP proxies
/// impossible to select.
#[gpui::test]
fn forward_modal_mode_chips_are_selectable(cx: &mut TestAppContext) {
    let config = init_config_dir();
    write_json(
        &config.join("ssh_profiles.json"),
        &serde_json::json!({"profiles": [{
            "id": "p", "name": "existing", "host": "127.0.0.1", "port": 22,
            "username": "root", "authType": "password", "privateKeyPath": "",
            "strictHostKeyChecking": false
        }]}),
    );
    write_json(&config.join("ssh_forwards.json"), &serde_json::json!({"forwards":[]}));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let services = Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), config.join("api.json")),
        ssh: SshService::new(runtime.handle().clone()),
        hosts: devbox::hosts::HostsService::with_path(config.join("hosts.json")),
    };
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));
    goto(&view, cx, ToolId::SshForward);

    let add = cx.debug_bounds("ssh_forward_add_button").expect("add button");
    cx.simulate_click(add.center(), Modifiers::none());
    assert!(cx.update(|_, app| view.read(app).pages.ssh_forward.show_form));

    let mode = |cx: &mut VisualTestContext| {
        cx.update(|_, app| view.read(app).pages.ssh_forward.form.mode.clone())
    };
    assert_eq!(mode(cx), "local");

    // Each chip must render and must be individually clickable.
    for (selector, expected) in [
        ("ssh_forward_mode_socks", "socks"),
        ("ssh_forward_mode_http", "http"),
        ("ssh_forward_mode_local", "local"),
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} chip was not rendered"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        assert_eq!(mode(cx), expected, "mode after clicking {selector}");
    }

    // Selecting a proxy mode should prefill the default proxy port.
    let bounds = cx.debug_bounds("ssh_forward_mode_socks").unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    let port = cx.update(|_, app| {
        view.read(app)
            .pages
            .ssh_forward
            .form
            .local_port
            .read(app)
            .text()
            .to_string()
    });
    assert_eq!(port, "1080");

    // The system-proxy checkbox must also be available for SOCKS5, not just
    // for the HTTP mode.
    let checkbox = cx
        .debug_bounds("ssh_forward_system_proxy")
        .expect("system proxy checkbox missing in socks mode");
    assert!(!cx.update(|_, app| view.read(app).pages.ssh_forward.form.system_proxy));
    cx.simulate_click(checkbox.center(), Modifiers::none());
    assert!(cx.update(|_, app| view.read(app).pages.ssh_forward.form.system_proxy));
}
