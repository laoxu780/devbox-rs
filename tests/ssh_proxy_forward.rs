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
        // SAFETY: called once before any DevBox service or view is created.
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
fn proxy_modes_can_be_selected_and_created(cx: &mut TestAppContext) {
    let config = init_config_dir();
    write_json(
        &config.join("ssh_profiles.json"),
        &serde_json::json!({"profiles": [{
            "id": "p", "name": "test", "host": "127.0.0.1", "port": 1,
            "username": "nobody", "authType": "password", "privateKeyPath": "",
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
    let (view, mut cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));
    goto(&view, &mut cx, ToolId::SshForward);

    for (index, (mode, expected_port, selector)) in [
        ("socks", 1080, "ssh_forward_mode_socks"),
        ("http", 8080, "ssh_forward_mode_http"),
    ]
    .into_iter()
    .enumerate()
    {
        // Open the add/edit form.
        let add = cx
            .debug_bounds("ssh_forward_add_button")
            .expect("add button");
        cx.simulate_click(add.center(), Modifiers::none());
        assert!(cx.update(|_, app| view.read(app).pages.ssh_forward.show_form));

        // Select the proxy mode chip.
        let chip = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{mode} mode chip was not rendered"));
        cx.simulate_click(chip.center(), Modifiers::none());
        assert_eq!(
            cx.update(|_, app| view.read(app).pages.ssh_forward.form.mode.clone()),
            mode,
            "selecting {mode} mode did not update the form",
        );
        assert_eq!(
            cx.update(|_, app| {
                view.read(app)
                    .pages
                    .ssh_forward
                    .form
                    .local_port
                    .read(app)
                    .text()
                    .to_string()
            }),
            expected_port.to_string(),
            "{mode} mode did not fill in the default port",
        );

        // Create the rule. The SSH connection will fail (no server on port 1),
        // but the rule must still be persisted with the chosen mode.
        let submit = cx
            .debug_bounds("ssh_forward_submit")
            .expect("submit button");
        cx.simulate_click(submit.center(), Modifiers::none());
        assert!(!cx.update(|_, app| view.read(app).pages.ssh_forward.show_form));

        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(config.join("ssh_forwards.json")).unwrap())
                .unwrap();
        let rules = stored["forwards"].as_array().unwrap();
        assert_eq!(rules.len(), index + 1);
        let rule = &rules[index];
        assert_eq!(rule["mode"], mode);
        assert_eq!(rule["localPort"], expected_port);

        // Reset the form state for the next iteration by closing any leftover
        // rule from the list view (the next click toggles the add button).
    }
}
