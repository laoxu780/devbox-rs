pub mod api_proxy;
pub mod base64;
pub mod code;
pub mod dashboard;
pub mod diff;
pub mod hash;
pub mod hosts;
pub mod json;
pub mod regex;
pub mod ssh_credentials;
pub mod ssh_forward;
pub mod ssh_keys;
pub mod timestamp;
pub mod uuid;

use gpui::{App, Context, Div, Entity, IntoElement, MouseDownEvent, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;
use crate::ui::app::DevBoxApp;
use crate::ui::text_input::TextInput;

pub use api_proxy::{ApiProxyState, ProxyForm, TargetForm};
pub use base64::Base64State;
pub use code::CodeState;
pub use code::decode_file;
pub use dashboard::DashboardState;
pub use diff::DiffState;
pub use hash::HashState;
pub use hosts::HostsState;
pub use json::JsonState;
pub use regex::RegexState;
pub use ssh_credentials::{ProfileForm, SshCredentialsState};
pub use ssh_forward::{ForwardForm, SshForwardState};
pub use ssh_keys::SshKeysState;
pub use timestamp::TimestampState;
pub use uuid::UuidState;

pub struct Pages {
    pub dashboard: DashboardState,
    pub json: JsonState,
    pub base64: Base64State,
    pub timestamp: TimestampState,
    pub uuid: UuidState,
    pub hash: HashState,
    pub regex: RegexState,
    pub diff: DiffState,
    pub code: CodeState,
    pub ssh_keys: SshKeysState,
    pub ssh_credentials: SshCredentialsState,
    pub ssh_forward: SshForwardState,
    pub api_proxy: ApiProxyState,
    pub hosts: HostsState,
}

impl Pages {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            dashboard: DashboardState::default(),
            json: JsonState::new(cx),
            base64: Base64State::new(cx),
            timestamp: TimestampState::new(cx),
            uuid: UuidState::new(),
            hash: HashState::new(cx),
            regex: RegexState::new(cx),
            diff: DiffState::new(cx),
            code: CodeState::new(cx),
            ssh_keys: SshKeysState::new(cx),
            ssh_credentials: SshCredentialsState::new(cx),
            ssh_forward: SshForwardState::new(cx),
            api_proxy: ApiProxyState::new(cx),
            hosts: HostsState::new(cx),
        }
    }

    pub fn inputs(&self) -> Vec<Entity<TextInput>> {
        let mut inputs = vec![
            self.json.input.clone(),
            self.base64.input.clone(),
            self.timestamp.input.clone(),
            self.hash.input.clone(),
            self.regex.pattern.clone(),
            self.regex.text.clone(),
            self.diff.left.clone(),
            self.diff.right.clone(),
            self.code.text.clone(),
            self.ssh_keys.name.clone(),
            self.ssh_keys.passphrase.clone(),
            self.ssh_credentials.form.name.clone(),
            self.ssh_credentials.form.host.clone(),
            self.ssh_credentials.form.port.clone(),
            self.ssh_credentials.form.username.clone(),
            self.ssh_credentials.form.key_path.clone(),
            self.ssh_credentials.form.secret.clone(),
            self.ssh_forward.form.name.clone(),
            self.ssh_forward.form.local_host.clone(),
            self.ssh_forward.form.local_port.clone(),
            self.ssh_forward.form.remote_host.clone(),
            self.ssh_forward.form.remote_port.clone(),
            self.api_proxy.form.name.clone(),
            self.api_proxy.form.host.clone(),
            self.api_proxy.form.port.clone(),
            self.api_proxy.form.strip.clone(),
            self.hosts.form.name.clone(),
            self.hosts.form.entries.clone(),
        ];
        for target in &self.api_proxy.form.targets {
            inputs.push(target.name.clone());
            inputs.push(target.url.clone());
        }
        inputs
    }
}

// ---------------------------------------------------------------------------
// Shared layout helpers
// ---------------------------------------------------------------------------

pub fn tool_page(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .max_w(px(1080.0))
        .w_full()
        .pb(px(20.0))
        .text_color(theme.text())
}

pub fn panel_actions(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_color(theme.muted())
}

pub fn text_field(theme: &Theme, input: &Entity<TextInput>, height: f32) -> Div {
    div()
        .h(px(height))
        .w_full()
        .bg(theme.card())
        .border_1()
        .border_color(theme.line())
        .rounded(px(8.0))
        .overflow_hidden()
        .child(input.clone())
}

pub fn text_area(theme: &Theme, input: &Entity<TextInput>, height: f32) -> Div {
    div()
        .h(px(height))
        .w_full()
        .bg(theme.card())
        .border_1()
        .border_color(theme.line())
        .rounded(px(9.0))
        .overflow_hidden()
        .child(input.clone())
}

pub fn io_head(
    theme: &Theme,
    title: impl Into<SharedString>,
    meta: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .pb(px(6.0))
        .text_size(px(11.0))
        .text_color(theme.muted())
        .child(
            div()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(title.into()),
        )
        .child(
            div()
                .text_color(theme.faint())
                .child(meta.into()),
        )
}

pub fn chip(theme: &Theme, label: impl Into<SharedString>, active: bool) -> Div {
    let label = label.into();
    let base = div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(26.0))
        .px(px(9.0))
        .rounded(px(6.0))
        .text_size(px(11.0))
        .cursor_pointer();
    if active {
        base.bg(theme.accent())
            .text_color(gpui::white())
            .font_weight(gpui::FontWeight::MEDIUM)
    } else {
        base.text_color(theme.muted())
            .hover(|style| style.bg(theme.accent_ghost()).text_color(theme.accent()))
    }
    .child(label)
}

pub fn segmented(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .p(px(2.0))
        .bg(theme.inset())
        .border_1()
        .border_color(theme.line())
        .rounded(px(8.0))
}

pub fn footer_line(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .pt(px(8.0))
        .text_size(px(10.5))
        .text_color(theme.faint())
}

pub fn mono(text: impl Into<SharedString>) -> Div {
    div()
        .font_family("Menlo")
        .text_size(px(11.5))
        .child(text.into())
}

pub fn spacer() -> Div {
    div().flex_1()
}

pub fn hstack(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_color(theme.text())
}

pub fn vstack() -> Div {
    div().flex().flex_col().gap(px(8.0))
}

/// Convert a `Div` into something the page dispatcher can box.
pub fn boxed(element: impl IntoElement) -> gpui::AnyElement {
    element.into_any_element()
}

/// Common shell for modal dialogs: dim the window, centre the content and
/// dismiss when the user clicks outside of it.
pub fn modal_shell(
    width: f32,
    selector: &'static str,
    on_outside: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    content: Div,
) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_center()
        .bg(gpui::rgba(0x00000055))
        .child(
            content
                .w(px(width))
                .debug_selector(move || selector.into())
                .id(selector)
                .on_mouse_down_out(on_outside),
        )
}
