//! DevBox desktop entry point: a GPUI window backed by the Tokio services.

use std::rc::Rc;

use gpui::{App, Application, KeyBinding};

use devbox::hosts::HostsService;
use devbox::proxy::ProxyService;
use devbox::ssh::SshService;
use devbox::system::{AppRuntime, install_alt_space_hotkey, open_main_window, restore_window};
use devbox::ui::app::{
    Dismiss, OpenPalette, Quit, Services, ToolHosts, ToolBase64, ToolHash, ToolJson, ToolTimestamp, ToolUuid,
};
use devbox::ui::text_input;

fn main() {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("无法创建 Tokio 运行时");
    let handle = tokio_runtime.handle().clone();
    let proxy = ProxyService::new(handle.clone());
    let ssh = SshService::new(handle);
    let services = Services {
        proxy: proxy.clone(),
        ssh: ssh.clone(),
        hosts: HostsService::new(),
    };
    let runtime = Rc::new(AppRuntime::new(services));

    let app = Application::new();
    // Clicking the dock icon restores the window after the user closed it.
    let reopen_runtime = runtime.clone();
    app.on_reopen(move |cx| restore_window(cx, &reopen_runtime, false));

    let proxy_for_quit = proxy.clone();
    let ssh_for_quit = ssh.clone();
    let hotkey_runtime = runtime.clone();

    app.run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-k", OpenPalette, None),
            KeyBinding::new("escape", Dismiss, None),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-1", ToolJson, None),
            KeyBinding::new("cmd-2", ToolBase64, None),
            KeyBinding::new("cmd-3", ToolTimestamp, None),
            KeyBinding::new("cmd-4", ToolUuid, None),
            KeyBinding::new("cmd-5", ToolHash, None),
            KeyBinding::new("cmd-6", ToolHosts, None),
            KeyBinding::new("backspace", text_input::Backspace, Some("TextInput")),
            KeyBinding::new("delete", text_input::Delete, Some("TextInput")),
            KeyBinding::new("left", text_input::Left, Some("TextInput")),
            KeyBinding::new("right", text_input::Right, Some("TextInput")),
            KeyBinding::new("up", text_input::Up, Some("TextInput")),
            KeyBinding::new("down", text_input::Down, Some("TextInput")),
            KeyBinding::new("shift-left", text_input::SelectLeft, Some("TextInput")),
            KeyBinding::new("shift-right", text_input::SelectRight, Some("TextInput")),
            KeyBinding::new("home", text_input::Home, Some("TextInput")),
            KeyBinding::new("end", text_input::End, Some("TextInput")),
            KeyBinding::new("cmd-a", text_input::SelectAll, Some("TextInput")),
            KeyBinding::new("cmd-v", text_input::Paste, Some("TextInput")),
            KeyBinding::new("cmd-c", text_input::Copy, Some("TextInput")),
            KeyBinding::new("cmd-x", text_input::Cut, Some("TextInput")),
        ]);

        open_main_window(cx, &runtime, false);

        if !install_alt_space_hotkey(hotkey_runtime.clone(), cx.to_async()) {
            eprintln!("Alt+Space 全局快捷键注册失败，可继续使用窗口内的 ⌘K 搜索");
        }

        cx.on_app_quit(move |_cx| {
            let proxy = proxy_for_quit.clone();
            let ssh = ssh_for_quit.clone();
            async move {
                proxy.close();
                ssh.close();
            }
        })
        .detach();

        // Keep the runtime alive for the whole application lifetime. Dropping it
        // would abort every proxy listener and SSH tunnel.
        std::mem::forget(tokio_runtime);
    });
}
