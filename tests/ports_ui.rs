use gpui::{Modifiers, TestAppContext, VisualTestContext};

use devbox::proxy::ProxyService;
use devbox::ssh::SshService;
use devbox::ui::app::{DevBoxApp, Services, ToolId};

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

fn goto(view: &gpui::Entity<DevBoxApp>, cx: &mut VisualTestContext, tool: ToolId) {
    // The sidebar scrolls, and the newest tool may be below the test window's
    // visible area, so select it directly.
    cx.update(|_, app| {
        view.update(app, |view, cx| view.select_tool(tool, cx));
    });
    assert_eq!(cx.update(|_, app| view.read(app).tool), tool);
}

#[gpui::test]
fn port_checker_reports_busy_and_free_ports(cx: &mut TestAppContext) {
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
    let (view, mut cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));
    goto(&view, &mut cx, ToolId::Ports);

    // Occupy a port for the duration of the check.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let busy_port = listener.local_addr().unwrap().port();
    // Pick a port that is very unlikely to be in use.
    let free_port = 49152u16;

    cx.update(|_, app| {
        let query = view.read(app).pages.ports.query.clone();
        query.update(app, |input, cx| {
            input.set_text(format!("{busy_port}, {free_port}"), cx)
        });
    });
    let check = cx.debug_bounds("ports_check").expect("check button");
    cx.simulate_click(check.center(), Modifiers::none());

    let checks = cx.update(|_, app| view.read(app).pages.ports.checks.clone());
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].port, busy_port);
    assert!(!checks[0].available, "occupied port reported as free");
    assert_eq!(checks[1].port, free_port);
    assert!(checks[1].available, "free port reported as busy");
}

#[gpui::test]
fn port_list_can_be_filtered_by_protocol(cx: &mut TestAppContext) {
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
    let (view, mut cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));
    goto(&view, &mut cx, ToolId::Ports);

    let chip = cx.debug_bounds("ports_protocol_udp").expect("UDP chip");
    cx.simulate_click(chip.center(), Modifiers::none());
    assert_eq!(
        cx.update(|_, app| view.read(app).pages.ports.protocol.clone()),
        "udp",
    );
}
