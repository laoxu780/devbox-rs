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
        // SAFETY: called once before any DevBox service or view is created.
        unsafe { std::env::set_var("DEVBOX_CONFIG_DIR", path) };
    });
}

fn services(dir: &std::path::Path, runtime: &tokio::runtime::Runtime) -> Services {
    Services {
        proxy: ProxyService::new_with_path(runtime.handle().clone(), dir.join("api_proxy.json")),
        ssh: SshService::new_with_paths(
            runtime.handle().clone(),
            dir.join("ssh_profiles.json"),
            dir.join("ssh_forwards.json"),
        ),
        hosts: devbox::hosts::HostsService::with_path(dir.join("hosts.json")),
    }
}

#[gpui::test]
fn sidebar_switches_tools(cx: &mut TestAppContext) {
    init_config_dir();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let services = services(temp.path(), &runtime);
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));

    // Sidebar starts below the 52px top bar. Probe a column of points and
    // record which tool each click selects.
    let mut seen = Vec::new();
    for y in (80..760).step_by(6) {
        cx.simulate_click(point(px(100.0), px(y as f32)), Modifiers::none());
        let tool = cx.update(|_, app| view.read(app).tool);
        seen.push((y, tool));
    }
    let reached: Vec<ToolId> = {
        let mut out = Vec::new();
        for (_, tool) in &seen {
            if !out.contains(tool) {
                out.push(*tool);
            }
        }
        out
    };
    assert!(
        reached.contains(&ToolId::Json),
        "clicking the sidebar never reached JSON; reached {reached:?}; samples {seen:?}"
    );
    assert!(
        reached.contains(&ToolId::ApiProxy),
        "clicking the sidebar never reached API proxy; reached {reached:?}"
    );
}

#[gpui::test]
fn text_input_focuses_and_accepts_text(cx: &mut TestAppContext) {
    init_config_dir();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let services = services(temp.path(), &runtime);
    let (view, cx) = cx.add_window_view(|_, cx| DevBoxApp::new(services, cx));

    // JSON page has a large text area on the right.
    cx.simulate_click(point(px(100.0), px(206.0)), Modifiers::none());
    let tool = cx.update(|_, app| view.read(app).tool);
    assert_eq!(tool, ToolId::Json, "expected JSON page, got {tool:?}");

    cx.simulate_click(point(px(500.0), px(180.0)), Modifiers::none());
    cx.simulate_input("hello");
    let text = cx.update(|_, app| view.read(app).pages.json.input.read(app).text().to_string());
    assert!(text.contains("hello"), "typing did not reach the editor: {text:?}");
}

