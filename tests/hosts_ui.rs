use gpui::{Modifiers, TestAppContext, point, px};

use devbox::hosts::{self, HostsService};
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
fn hosts_profiles_can_be_created(cx: &mut TestAppContext) {
    let config = init_config_dir();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let hosts_path = config.join("hosts_profiles.json");
    let hosts = HostsService::with_path(hosts_path.clone());
    let services = Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), config.join("api.json")),
        ssh: SshService::new_with_paths(
            runtime.handle().clone(),
            config.join("profiles.json"),
            config.join("forwards.json"),
        ),
        hosts: hosts.clone(),
    };
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));

    let mut navigated = false;
    for y in (60..900).step_by(6) {
        cx.simulate_click(point(px(100.0), px(y as f32)), Modifiers::none());
        if cx.update(|_, app| view.read(app).tool) == ToolId::Hosts {
            navigated = true;
            break;
        }
    }
    assert!(navigated, "could not reach Hosts page");

    let add = cx.debug_bounds("hosts_add_button").expect("add button");
    cx.simulate_click(add.center(), Modifiers::none());
    assert!(cx.debug_bounds("hosts_form_modal").is_some());

    view.update(cx, |app, cx| {
        app.pages
            .hosts
            .form
            .name
            .update(cx, |input, cx| input.set_text("dev", cx));
        app.pages.hosts.form.entries.update(cx, |input, cx| {
            input.set_text("10.9.8.7 api.dev.local", cx)
        });
    });
    let save = cx.debug_bounds("hosts_form_save").expect("save button");
    cx.simulate_click(save.center(), Modifiers::none());

    assert_eq!(hosts.list().len(), 1);
    assert_eq!(hosts.list()[0].name, "dev");
    assert!(hosts.list()[0].entries[0].ip == "10.9.8.7");
    assert!(!cx.update(|_, app| view.read(app).pages.hosts.show_form));

    let preview = hosts.preview(&hosts.list()[0].id).unwrap();
    assert!(preview.contains(hosts::BLOCK_START));
    assert!(preview.contains("10.9.8.7\tapi.dev.local"));
}
