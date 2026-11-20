use std::io::Write;

use gpui::{Modifiers, TestAppContext, point, px};

use devbox::proxy::ProxyService;
use devbox::ssh::SshService;
use devbox::ui::app::{DevBoxApp, Services};

fn init_config_dir() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);
        // SAFETY: called once before any DevBox service or view is created.
        unsafe { std::env::set_var("DEVBOX_CONFIG_DIR", path) };
    });
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) {
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(value.to_string().as_bytes()).unwrap();
}

#[gpui::test]
fn forward_switch_disables_the_rule(cx: &mut TestAppContext) {
    init_config_dir();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let profiles_path = dir.path().join("ssh_profiles.json");
    let forwards_path = dir.path().join("ssh_forwards.json");
    write_json(
        &profiles_path,
        &serde_json::json!({"profiles": [{
            "id": "p", "name": "test", "host": "127.0.0.1", "port": 1,
            "username": "nobody", "authType": "password", "privateKeyPath": "",
            "strictHostKeyChecking": false
        }]}),
    );
    write_json(
        &forwards_path,
        &serde_json::json!({"forwards": [{
            "id": "f", "profileID": "p", "name": "test forward",
            "localHost": "127.0.0.1", "localPort": 0,
            "remoteHost": "127.0.0.1", "remotePort": 80,
            "disabled": false, "mode": "local", "position": 1
        }]}),
    );
    let ssh = SshService::new_with_paths(
        runtime.handle().clone(),
        profiles_path.clone(),
        forwards_path.clone(),
    );
    let services = Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), dir.path().join("api.json")),
        ssh: ssh.clone(),
        hosts: devbox::hosts::HostsService::with_path(dir.path().join("hosts.json")),
    };
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));

    // Navigate to the SSH forward page via the sidebar (the exact geometry
    // depends on the test platform window size).
    let mut navigated = false;
    for y in (60..760).step_by(6) {
        cx.simulate_click(point(px(100.0), px(y as f32)), Modifiers::none());
        if cx.update(|_, app| view.read(app).tool)
            == devbox::ui::app::ToolId::SshForward
        {
            navigated = true;
            break;
        }
    }
    assert!(navigated, "could not navigate to SSH forward page");
    assert_ne!(ssh.list_forwards()[0].status, "stopped");

    // Click the rule's on/off switch. Its debug selector keeps the test
    // independent of layout changes.
    let bounds = cx
        .debug_bounds("ssh_switch")
        .expect("forward switch was not rendered");
    let center = bounds.center();
    cx.simulate_click(center, Modifiers::none());

    assert_eq!(ssh.list_forwards()[0].status, "stopped");
    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&forwards_path).unwrap()).unwrap();
    assert_eq!(stored["forwards"][0]["disabled"], true);
}
