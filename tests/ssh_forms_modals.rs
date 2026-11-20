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

#[gpui::test]
fn credential_form_is_a_modal(cx: &mut TestAppContext) {
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
    goto(&view, cx, ToolId::SshCredentials);

    let add = cx.debug_bounds("ssh_credentials_add_button").expect("add button");
    cx.simulate_click(add.center(), Modifiers::none());
    assert!(cx.update(|_, app| view.read(app).pages.ssh_credentials.show_form));
    assert!(cx.debug_bounds("ssh_credentials_dialog").is_some());
    let close = cx.debug_bounds("ssh_credentials_dialog_close").unwrap();
    cx.simulate_click(close.center(), Modifiers::none());
    assert!(!cx.update(|_, app| view.read(app).pages.ssh_credentials.show_form));
}

#[gpui::test]
fn forward_form_is_a_modal(cx: &mut TestAppContext) {
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
    assert!(cx.debug_bounds("ssh_forward_dialog").is_some());
    let close = cx.debug_bounds("ssh_forward_dialog_close").unwrap();
    cx.simulate_click(close.center(), Modifiers::none());
    assert!(!cx.update(|_, app| view.read(app).pages.ssh_forward.show_form));
}

#[gpui::test]
fn api_proxy_form_is_a_modal(cx: &mut TestAppContext) {
    let config = init_config_dir();
    write_json(&config.join("api_proxy.json"), &serde_json::json!({"routes":[]}));
    write_json(&config.join("ssh_profiles.json"), &serde_json::json!({"profiles":[]}));
    write_json(&config.join("ssh_forwards.json"), &serde_json::json!({"forwards":[]}));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let services = Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), config.join("api_proxy.json")),
        ssh: SshService::new(runtime.handle().clone()),
        hosts: devbox::hosts::HostsService::with_path(config.join("hosts.json")),
    };
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));
    goto(&view, cx, ToolId::ApiProxy);

    let add = cx.debug_bounds("api_proxy_add_button").expect("add button");
    cx.simulate_click(add.center(), Modifiers::none());
    assert!(cx.update(|_, app| view.read(app).pages.api_proxy.show_form));
    assert!(cx.debug_bounds("api_proxy_dialog").is_some());
    let close = cx.debug_bounds("api_proxy_dialog_close").unwrap();
    cx.simulate_click(close.center(), Modifiers::none());
    assert!(!cx.update(|_, app| view.read(app).pages.api_proxy.show_form));
}
