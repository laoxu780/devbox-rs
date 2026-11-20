use gpui::{Modifiers, TestAppContext, point};

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

#[gpui::test]
fn ssh_key_modals_open_and_close(cx: &mut TestAppContext) {
    let config = init_config_dir();
    let keys_dir = config.join("keys");
    std::fs::create_dir_all(&keys_dir).unwrap();
    let status = std::process::Command::new("ssh-keygen")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .args(["-t", "ed25519", "-f"])
        .arg(keys_dir.join("id_test_modal"))
        .args(["-N", "", "-C", "devbox-test"])
        .status()
        .unwrap();
    assert!(status.success());

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

    // Navigate to SSH keys.
    let mut navigated = false;
    for y in (60..760).step_by(6) {
        cx.simulate_click(point(gpui::px(100.0), gpui::px(y as f32)), Modifiers::none());
        if cx.update(|_, app| view.read(app).tool) == ToolId::SshKeys {
            navigated = true;
            break;
        }
    }
    assert!(navigated, "could not reach SSH keys page");

    // Generate dialog.
    let bounds = cx.debug_bounds("ssh_generate_button").expect("generate button");
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert!(cx.update(|_, app| view.read(app).pages.ssh_keys.show_form));
    assert!(cx.debug_bounds("ssh_generate_dialog").is_some());
    let close = cx.debug_bounds("ssh_generate_close").unwrap();
    cx.simulate_click(close.center(), Modifiers::none());
    assert!(!cx.update(|_, app| view.read(app).pages.ssh_keys.show_form));

    // Public-key dialog.
    let bounds = cx
        .debug_bounds("ssh_view_pub_id_test_modal")
        .expect("view public key button");
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert!(cx
        .update(|_, app| view.read(app).pages.ssh_keys.public_detail.is_some()));
    assert!(cx.debug_bounds("ssh_public_dialog").is_some());
    let close = cx.debug_bounds("ssh_public_close").unwrap();
    cx.simulate_click(close.center(), Modifiers::none());
    assert!(cx
        .update(|_, app| view.read(app).pages.ssh_keys.public_detail.is_none()));
}
