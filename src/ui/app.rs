//! Root view: top bar, grouped sidebar, command palette and tool routing.

use std::time::{Duration, Instant};

use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseDownEvent, Render, SharedString, Window, div, prelude::*, px,
};

use crate::hosts::HostsService;
use crate::proxy::ProxyService;
use crate::ssh::SshService;
use crate::store::UiSettings;
use crate::theme::{Theme, ThemeMode};
use crate::ui::pages::Pages;
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ButtonKind, badge, button, dot, switch};

gpui::actions!(
    devbox,
    [
        OpenPalette,
        Dismiss,
        Confirm,
        Quit,
        ToolJson,
        ToolBase64,
        ToolTimestamp,
        ToolUuid,
        ToolHash,
        ToolHosts,
        ToolPorts,
    ]
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolId {
    Dashboard,
    Json,
    Base64,
    Timestamp,
    Uuid,
    Hash,
    Regex,
    Diff,
    Code,
    SshKeys,
    SshCredentials,
    SshForward,
    ApiProxy,
    Hosts,
    Ports,
}

impl ToolId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Json => "json",
            Self::Base64 => "base64",
            Self::Timestamp => "timestamp",
            Self::Uuid => "uuid",
            Self::Hash => "hash",
            Self::Regex => "regex",
            Self::Diff => "diff",
            Self::Code => "code",
            Self::SshKeys => "ssh-keys",
            Self::SshCredentials => "ssh-credentials",
            Self::SshForward => "ssh-forward",
            Self::ApiProxy => "api-proxy",
            Self::Hosts => "hosts",
            Self::Ports => "ports",
        }
    }
}

pub struct ToolMeta {
    pub id: ToolId,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub glyph: &'static str,
    pub accent: u32,
    pub shortcut: Option<&'static str>,
}

pub const VIOLET: u32 = 0x6d5bd0;
pub const LIME: u32 = 0x6a9a2f;
pub const CYAN: u32 = 0x12939b;
pub const ORANGE: u32 = 0xc27a2c;
pub const PINK: u32 = 0xc2548f;
pub const BLUE: u32 = 0x3f74c9;
pub const RED: u32 = 0xc0504d;

pub const TOOLS: &[ToolMeta] = &[
    ToolMeta {
        id: ToolId::Dashboard,
        name: "仪表盘",
        description: "总览与快捷开关",
        category: "总览",
        glyph: "D",
        accent: VIOLET,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::Json,
        name: "JSON 工具",
        description: "格式化、压缩与校验 JSON",
        category: "数据处理",
        glyph: "{}",
        accent: LIME,
        shortcut: Some("⌘1"),
    },
    ToolMeta {
        id: ToolId::Base64,
        name: "Base64 编解码",
        description: "文本与 Base64 快速互转",
        category: "编码解码",
        glyph: "64",
        accent: CYAN,
        shortcut: Some("⌘2"),
    },
    ToolMeta {
        id: ToolId::Timestamp,
        name: "时间戳转换",
        description: "Unix 时间戳与日期互转",
        category: "生成器",
        glyph: "⏱",
        accent: ORANGE,
        shortcut: Some("⌘3"),
    },
    ToolMeta {
        id: ToolId::Uuid,
        name: "UUID 生成器",
        description: "生成随机 UUID v4",
        category: "生成器",
        glyph: "ID",
        accent: PINK,
        shortcut: Some("⌘4"),
    },
    ToolMeta {
        id: ToolId::Hash,
        name: "Hash 计算",
        description: "计算 MD5、SHA-1、SHA-256",
        category: "编码解码",
        glyph: "#",
        accent: VIOLET,
        shortcut: Some("⌘5"),
    },
    ToolMeta {
        id: ToolId::Regex,
        name: "正则测试",
        description: "实时测试与匹配结果预览",
        category: "校验测试",
        glyph: ".*",
        accent: BLUE,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::Diff,
        name: "文本对比",
        description: "并排 / 合并视图查看文本差异",
        category: "数据处理",
        glyph: "±",
        accent: RED,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::Code,
        name: "二维码 / 条码",
        description: "生成多种码制，图片扫码解码",
        category: "数据处理",
        glyph: "▦",
        accent: VIOLET,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::SshKeys,
        name: "SSH 密钥",
        description: "管理公私钥：查看、生成、导出",
        category: "网络工具",
        glyph: "K",
        accent: VIOLET,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::SshCredentials,
        name: "SSH 凭证",
        description: "管理服务器连接凭证与密钥",
        category: "网络工具",
        glyph: "C",
        accent: CYAN,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::SshForward,
        name: "SSH 端口转发",
        description: "将服务器内网端口安全映射到本地",
        category: "网络工具",
        glyph: "→",
        accent: CYAN,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::ApiProxy,
        name: "接口环境转发",
        description: "本地端口一键切换开发 / 生产后端",
        category: "网络工具",
        glyph: "⇄",
        accent: BLUE,
        shortcut: None,
    },
    ToolMeta {
        id: ToolId::Hosts,
        name: "Hosts 环境",
        description: "一键切换本机域名解析环境",
        category: "网络工具",
        glyph: "H",
        accent: LIME,
        shortcut: Some("⌘6"),
    },
    ToolMeta {
        id: ToolId::Ports,
        name: "端口检测",
        description: "查看监听端口，检测端口是否可用",
        category: "网络工具",
        glyph: "P",
        accent: ORANGE,
        shortcut: Some("⌘7"),
    },
];

pub const CATEGORY_ORDER: [&str; 6] = ["总览", "数据处理", "编码解码", "生成器", "校验测试", "网络工具"];

pub fn tool_meta(id: ToolId) -> &'static ToolMeta {
    TOOLS.iter().find(|tool| tool.id == id).unwrap()
}

#[derive(Clone)]
pub struct Services {
    pub proxy: ProxyService,
    pub ssh: SshService,
    pub hosts: HostsService,
}

pub struct Toast {
    message: String,
    error: bool,
    stamp: Instant,
}

pub enum ConfirmAction {
    DeleteProxyRoute(String),
    DeleteProfile(String),
    DeleteForward(String),
    DeleteKey(String),
    DeleteHostsProfile(String),
    ClearHosts,
}

pub struct ConfirmState {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

pub struct DevBoxApp {
    pub services: Services,
    pub settings: UiSettings,
    pub theme_mode: ThemeMode,
    pub theme: Theme,
    pub tool: ToolId,
    pub pages: Pages,
    pub palette: Option<Entity<TextInput>>,
    pub palette_index: usize,
    pub toast: Option<Toast>,
    pub confirm: Option<ConfirmState>,
    pub hidden_panel: bool,
    pub focus_handle: FocusHandle,
    pub focus_palette: bool,
    pub now: Instant,
}

impl DevBoxApp {
    pub fn new(services: Services, cx: &mut Context<Self>) -> Self {
        let mut settings = UiSettings::load();
        let theme_mode = ThemeMode::from_str(&settings.theme);
        let restored = TOOLS
            .iter()
            .find(|tool| tool.id.as_str() == settings.last_tool)
            .map(|tool| tool.id)
            .unwrap_or(ToolId::Dashboard);
        let tool = if settings.hidden_tools.iter().any(|id| id == restored.as_str()) {
            ToolId::Dashboard
        } else {
            restored
        };
        settings.last_tool = tool.as_str().to_string();
        let pages = Pages::new(cx);
        let app = Self {
            services,
            settings,
            theme_mode,
            theme: Theme::new(false),
            tool,
            pages,
            palette: None,
            palette_index: 0,
            toast: None,
            confirm: None,
            hidden_panel: false,
            focus_handle: cx.focus_handle(),
            focus_palette: false,
            now: Instant::now(),
        };
        for input in app.pages.inputs() {
            cx.observe(&input, |_, _, cx| cx.notify()).detach();
        }
        app
    }

    pub fn select_tool(&mut self, tool: ToolId, cx: &mut Context<Self>) {
        self.tool = tool;
        self.settings.last_tool = tool.as_str().to_string();
        self.settings.save();
        self.palette = None;
        self.palette_index = 0;
        self.pages.ssh_keys.show_form = false;
        self.pages.ssh_keys.public_detail = None;
        self.pages.ssh_credentials.show_form = false;
        self.pages.ssh_forward.show_form = false;
        self.pages.api_proxy.show_form = false;
        self.pages.hosts.show_form = false;
        cx.notify();
    }

    pub fn notify(&mut self, message: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        let stamp = Instant::now();
        self.toast = Some(Toast {
            message: message.into(),
            error,
            stamp,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2600))
                .await;
            this.update(cx, |app, cx| {
                if app.toast.as_ref().map(|toast| toast.stamp) == Some(stamp) {
                    app.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn copy(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.notify("已复制到剪贴板", false, cx);
    }

    pub fn is_hidden(&self, id: ToolId) -> bool {
        self.settings
            .hidden_tools
            .iter()
            .any(|hidden| hidden == id.as_str())
    }

    pub fn toggle_hidden(&mut self, id: ToolId, cx: &mut Context<Self>) {
        if let Some(index) = self
            .settings
            .hidden_tools
            .iter()
            .position(|hidden| hidden == id.as_str())
        {
            self.settings.hidden_tools.remove(index);
        } else {
            self.settings.hidden_tools.push(id.as_str().to_string());
        }
        self.settings.save();
        if self.is_hidden(self.tool) {
            self.tool = ToolId::Dashboard;
            self.settings.last_tool = self.tool.as_str().to_string();
            self.settings.save();
        }
        cx.notify();
    }

    pub fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_mode = self.theme_mode.next();
        self.settings.theme = self.theme_mode.as_str().to_string();
        self.settings.save();
        cx.notify();
    }

    pub fn open_palette(&mut self, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            return;
        }
        let input = cx.new(|cx| TextInput::single(cx, "搜索工具、功能或命令…"));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        self.palette = Some(input);
        self.palette_index = 0;
        self.focus_palette = true;
        cx.notify();
    }

    /// Global shortcut entry point: show the palette when closed, dismiss it
    /// when already open.
    pub fn toggle_palette(&mut self, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.palette = None;
            self.palette_index = 0;
            self.focus_palette = false;
        } else {
            self.open_palette(cx);
            return;
        }
        cx.notify();
    }

    fn close_overlays(&mut self, cx: &mut Context<Self>) {
        let had_palette = self.palette.take().is_some();
        let had_confirm = self.confirm.take().is_some();
        let had_keys_modal = self.pages.ssh_keys.show_form
            || self.pages.ssh_keys.public_detail.is_some();
        let had_credential_modal = self.pages.ssh_credentials.show_form;
        let had_forward_modal = self.pages.ssh_forward.show_form;
        let had_proxy_modal = self.pages.api_proxy.show_form;
        let had_hosts_modal = self.pages.hosts.show_form;
        self.pages.ssh_keys.show_form = false;
        self.pages.ssh_keys.public_detail = None;
        self.pages.ssh_credentials.show_form = false;
        self.pages.ssh_forward.show_form = false;
        self.pages.api_proxy.show_form = false;
        self.pages.hosts.show_form = false;
        if had_palette
            || had_confirm
            || self.hidden_panel
            || had_keys_modal
            || had_credential_modal
            || had_forward_modal
            || had_proxy_modal
            || had_hosts_modal
        {
            self.hidden_panel = false;
            cx.notify();
        }
    }

    fn palette_matches(&self, cx: &App) -> Vec<&'static ToolMeta> {
        let query = self
            .palette
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_lowercase())
            .unwrap_or_default();
        TOOLS
            .iter()
            .filter(|tool| !self.is_hidden(tool.id))
            .filter(|tool| {
                query.is_empty()
                    || tool.name.to_lowercase().contains(&query)
                    || tool.description.to_lowercase().contains(&query)
                    || tool.category.to_lowercase().contains(&query)
                    || tool
                        .shortcut
                        .map(|shortcut| shortcut.contains(&query))
                        .unwrap_or(false)
            })
            .collect()
    }

    fn on_open_palette(&mut self, _: &OpenPalette, _: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(cx);
    }

    fn on_dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.close_overlays(cx);
    }

    fn on_confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            if let Some(tool) = self.palette_matches(cx).first() {
                let id = tool.id;
                self.select_tool(id, cx);
            }
        } else if self.confirm.is_some() {
            self.run_confirm(cx);
        }
    }

    pub fn run_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        let result = match confirm.action {
            ConfirmAction::DeleteProxyRoute(id) => {
                self.services.proxy.delete_route(&id).map(|_| "接口转发已删除")
            }
            ConfirmAction::DeleteProfile(id) => {
                self.services.ssh.delete_profile(&id).map(|_| "SSH 凭证已删除")
            }
            ConfirmAction::DeleteForward(id) => {
                self.services.ssh.delete_forward(&id).map(|_| "SSH 转发已删除")
            }
            ConfirmAction::DeleteKey(path) => self.services.ssh.delete_key(&path).map(|_| "密钥已删除"),
            ConfirmAction::DeleteHostsProfile(id) => {
                self.services.hosts.delete_profile(&id).map(|_| "hosts 环境已删除")
            }
            ConfirmAction::ClearHosts => self.services.hosts.clear().map(|_| "hosts 配置已清除"),
        };
        match result {
            Ok(message) => self.notify(message, false, cx),
            Err(error) => self.notify(error, true, cx),
        }
    }

    fn on_tool_shortcut(&mut self, tool: ToolId, cx: &mut Context<Self>) {
        if !self.is_hidden(tool) {
            self.select_tool(tool, cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            self.close_overlays(cx);
        } else if key == "enter" && self.palette.is_some() {
            if let Some(tool) = self.palette_matches(cx).first() {
                let id = tool.id;
                self.select_tool(id, cx);
            }
        }
    }

    fn render_topbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(52.0))
            .px(px(18.0))
            .border_b_1()
            .border_color(theme.line())
            .bg(theme.canvas())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .min_w(px(240.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(28.0))
                            .rounded(px(8.0))
                            .bg(theme.accent())
                            .text_color(gpui::white())
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("D"),
                    )
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.text())
                            .child("DevBox"),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint())
                            .child("RS"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .text_size(px(12.0))
                    .text_color(theme.muted())
                    .child(dot(theme.ok))
                    .child(format!(
                        "{} · {}",
                        tool_meta(self.tool).name,
                        tool_meta(self.tool).category
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .min_w(px(240.0))
                    .justify_end()
                    .child(
                        button(
                            &theme,
                            format!("{} {}", theme_icon(self.theme_mode), self.theme_mode.label()),
                            ButtonKind::Ghost,
                        )
                        .id("app_auto_1")

                        .id("app_auto2_1")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| app.cycle_theme(cx))),
                    )
                    .child(
                        button(&theme, "工具设置", ButtonKind::Subtle)
                            .id("app_auto_2")

                            .id("app_auto2_2")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.hidden_panel = !app.hidden_panel;
                                cx.notify();
                            })),
                    )
                    .child(
                        button(&theme, "⌘K 搜索", ButtonKind::Ghost)
                            .id("app_auto_3")

                            .id("app_auto2_3")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.open_palette(cx)
                            })),
                    ),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut nav = div().flex().flex_col().gap(px(14.0)).pt(px(12.0));
        for category in CATEGORY_ORDER {
            let tools: Vec<&ToolMeta> = TOOLS
                .iter()
                .filter(|tool| tool.category == category && !self.is_hidden(tool.id))
                .collect();
            if tools.is_empty() {
                continue;
            }
            let mut group = div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .px(px(9.0))
                        .pb(px(4.0))
                        .text_size(px(10.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.faint())
                        .child(category),
                );
            for tool in tools {
                let active = tool.id == self.tool;
                let id = tool.id;
                let mut item = div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .h(px(34.0))
                    .px(px(8.0))
                    .rounded(px(9.0))
                    .cursor_pointer()
                    .text_size(px(12.5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.0))
                            .rounded(px(7.0))
                            .bg(gpui::rgba((tool.accent << 8) | 0x1f))
                            .text_color(gpui::rgb(tool.accent))
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(tool.glyph),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(tool.name),
                    );
                if let Some(shortcut) = tool.shortcut {
                    item = item.child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint())
                            .child(shortcut),
                    );
                }
                if active {
                    item = item
                        .bg(theme.accent_tint())
                        .text_color(theme.accent())
                        .font_weight(gpui::FontWeight::SEMIBOLD);
                } else {
                    item = item.text_color(theme.muted()).hover(|style| {
                        style
                            .bg(theme.accent_ghost())
                            .text_color(theme.text())
                            .cursor_pointer()
                    });
                }
                group = group.child(
                    item.id(crate::ui::id(format!("sidebar-{}", id.as_str())))
                        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                            app.select_tool(id, cx)
                        })),
                );
            }
            nav = nav.child(group);
        }

        div()
            .flex()
            .flex_col()
            .w(px(232.0))
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(theme.line())
            .p(px(12.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .h(px(34.0))
                    .px(px(10.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(9.0))
                    .text_size(px(12.0))
                    .text_color(theme.faint())
                    .cursor_pointer()
                    .hover(|style| style.border_color(theme.accent()))
                    .id("app_auto_4")

                    .id("app_auto2_5")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| app.open_palette(cx)))
                    .child("⌕")
                    .child(div().flex_1().child("搜索工具…"))
                    .child(
                        div()
                            .px(px(5.0))
                            .py(px(1.0))
                            .border_1()
                            .border_color(theme.line())
                            .rounded(px(4.0))
                            .text_size(px(9.0))
                            .child("⌘K"),
                    ),
            )
            .child(div().flex_1().min_h_0().id("app_auto2_6")
.overflow_y_scroll().child(nav))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(8.0))
                    .border_t_1()
                    .border_color(theme.line())
                    .text_size(px(10.0))
                    .text_color(theme.faint())
                    .child("v0.1.0 · Rust")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(dot(theme.ok))
                            .child("本地"),
                    ),
            )
    }

    fn render_status(&self) -> impl IntoElement {
        let theme = self.theme;
        let running_proxy = self
            .services
            .proxy
            .list_routes()
            .iter()
            .filter(|route| route.status == "running")
            .count();
        let running_ssh = self
            .services
            .ssh
            .list_forwards()
            .iter()
            .filter(|forward| forward.status == "running")
            .count();
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(26.0))
            .px(px(16.0))
            .border_t_1()
            .border_color(theme.line())
            .bg(theme.canvas())
            .text_size(px(10.5))
            .text_color(theme.faint())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(format!("接口转发 {running_proxy} 条运行中"))
                    .child(format!("SSH 转发 {running_ssh} 条运行中")),
            )
            .child("所有数据均在本机处理 · 不上传网络")
    }

    fn render_palette(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let matches = self.palette_matches(cx);
        let query = self
            .palette
            .as_ref()
            .map(|input| input.read(cx).text().to_lowercase())
            .unwrap_or_default();
        let mut list = div().flex().flex_col().gap(px(2.0)).max_h(px(360.0)).id("app_auto2_7")
.overflow_y_scroll();
        for (index, tool) in matches.iter().enumerate() {
            let id = tool.id;
            let selected = index == self.palette_index;
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .h(px(38.0))
                    .px(px(9.0))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .bg(if selected {
                        theme.accent_ghost()
                    } else {
                        theme.card()
                    })
                    .hover(|style| style.bg(theme.accent_ghost()))
                    .id(crate::ui::id(format!("palette-{}", tool.id.as_str())))
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.select_tool(id, cx)
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.0))
                            .rounded(px(7.0))
                            .bg(gpui::rgba((tool.accent << 8) | 0x1f))
                            .text_color(gpui::rgb(tool.accent))
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(tool.glyph),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(theme.text())
                                    .child(tool.name),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.faint())
                                    .text_ellipsis()
                                    .child(tool.description),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint())
                            .child(tool.category),
                    ),
            );
        }
        if matches.is_empty() {
            list = list.child(
                div()
                    .py(px(22.0))
                    .text_size(px(11.5))
                    .text_color(theme.faint())
                    .child(format!("没有匹配 “{query}” 的工具")),
            );
        }

        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(110.0))
            .bg(gpui::rgba(0x00000038))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .w(px(560.0))
                    .p(px(10.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line_strong())
                    .rounded(px(13.0))
                    .shadow_lg()
                    .id("app_auto_6")

                    .id("app_auto2_9")
.on_mouse_down_out(cx.listener(|app, _: &MouseDownEvent, _, cx| {
                        app.palette = None;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .h(px(38.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(theme.line())
                            .child(
                                self.palette
                                    .as_ref()
                                    .map(|input| input.clone().into_any_element())
                                    .unwrap_or_else(|| div().into_any_element()),
                            ),
                    )
                    .child(list)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .px(px(6.0))
                            .pt(px(2.0))
                            .text_size(px(9.5))
                            .text_color(theme.faint())
                            .child("↵ 打开")
                            .child("esc 关闭")
                            .child("⌘K 搜索"),
                    ),
            )
    }

    fn render_hidden_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut list = div().flex().flex_col().gap(px(2.0));
        for tool in TOOLS {
            if tool.id == ToolId::Dashboard {
                continue;
            }
            let hidden = self.is_hidden(tool.id);
            let id = tool.id;
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .h(px(36.0))
                    .px(px(8.0))
                    .rounded(px(8.0))
                    .hover(|style| style.bg(theme.accent_ghost()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(22.0))
                            .rounded(px(6.0))
                            .bg(gpui::rgba((tool.accent << 8) | 0x1f))
                            .text_color(gpui::rgb(tool.accent))
                            .text_size(px(9.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(tool.glyph),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(theme.text())
                            .child(tool.name),
                    )
                    .child(
                        switch(&theme, !hidden)
                            .id(crate::ui::id(format!("hidden-{}", tool.id.as_str())))
                            .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                                app.toggle_hidden(id, cx)
                            })),
                    ),
            );
        }
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(90.0))
            .bg(gpui::rgba(0x00000038))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .w(px(420.0))
                    .p(px(16.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line_strong())
                    .rounded(px(13.0))
                    .shadow_lg()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text())
                            .child("工具显示设置"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.faint())
                            .child("关闭后该工具会从侧边栏和命令面板中隐藏。"),
                    )
                    .child(list)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .child(
                                button(&theme, "完成", ButtonKind::Primary).id("app_auto2_11")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.hidden_panel = false;
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
    }

    fn render_confirm(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let (title, message) = self
            .confirm
            .as_ref()
            .map(|confirm| (confirm.title.clone(), confirm.message.clone()))
            .unwrap_or_default();
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000038))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .w(px(380.0))
                    .p(px(18.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line_strong())
                    .rounded(px(13.0))
                    .shadow_lg()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text())
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.muted())
                            .child(message),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                button(&theme, "取消", ButtonKind::Ghost).id("app_auto2_12")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.confirm = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "确认删除", ButtonKind::Danger).id("app_auto2_13")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| app.run_confirm(cx)),
                                ),
                            ),
                    ),
            )
    }

    fn render_toast(&self) -> impl IntoElement {
        let theme = self.theme;
        let (message, error) = self
            .toast
            .as_ref()
            .map(|toast| (toast.message.clone(), toast.error))
            .unwrap_or_default();
        let color = if error { theme.danger() } else { theme.ok() };
        div()
            .absolute()
            .bottom(px(44.0))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(34.0))
                    .px(px(14.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line_strong())
                    .rounded(px(9.0))
                    .shadow_lg()
                    .text_size(px(11.5))
                    .text_color(theme.text())
                    .child(div().size(px(6.0)).rounded_full().bg(color))
                    .child(message),
            )
    }

    fn render_page(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.tool {
            ToolId::Dashboard => self.render_dashboard(cx).into_any_element(),
            ToolId::Json => self.render_json(cx).into_any_element(),
            ToolId::Base64 => self.render_base64(cx).into_any_element(),
            ToolId::Timestamp => self.render_timestamp(cx).into_any_element(),
            ToolId::Uuid => self.render_uuid(cx).into_any_element(),
            ToolId::Hash => self.render_hash(cx).into_any_element(),
            ToolId::Regex => self.render_regex(cx).into_any_element(),
            ToolId::Diff => self.render_diff(cx).into_any_element(),
            ToolId::Code => self.render_code(cx).into_any_element(),
            ToolId::SshKeys => self.render_ssh_keys(cx).into_any_element(),
            ToolId::SshCredentials => self.render_ssh_credentials(cx).into_any_element(),
            ToolId::SshForward => self.render_ssh_forward(cx).into_any_element(),
            ToolId::ApiProxy => self.render_api_proxy(cx).into_any_element(),
            ToolId::Hosts => self.render_hosts(cx).into_any_element(),
            ToolId::Ports => self.render_ports(cx).into_any_element(),
        }
    }
}

fn theme_icon(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "◐",
        ThemeMode::Light => "☀",
        ThemeMode::Dark => "☾",
    }
}

impl Focusable for DevBoxApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DevBoxApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = self.theme_mode.resolve(window);
        self.now = Instant::now();
        if self.focus_palette {
            if let Some(input) = self.palette.clone() {
                let handle = input.read(cx).focus_handle();
                window.focus(&handle);
            }
            self.focus_palette = false;
        }
        let theme = self.theme;
        let has_palette = self.palette.is_some();
        let has_confirm = self.confirm.is_some();
        let has_hidden = self.hidden_panel;
        let has_toast = self.toast.is_some();
        let keys_generate_modal = self.tool == ToolId::SshKeys && self.pages.ssh_keys.show_form;
        let keys_public_modal =
            self.tool == ToolId::SshKeys && self.pages.ssh_keys.public_detail.is_some();
        let credentials_modal =
            self.tool == ToolId::SshCredentials && self.pages.ssh_credentials.show_form;
        let forward_modal =
            self.tool == ToolId::SshForward && self.pages.ssh_forward.show_form;
        let proxy_modal = self.tool == ToolId::ApiProxy && self.pages.api_proxy.show_form;
        let hosts_modal = self.tool == ToolId::Hosts && self.pages.hosts.show_form;
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.canvas())
            .text_color(theme.text())
            .text_size(px(13.0))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_open_palette))
            .on_action(cx.listener(Self::on_dismiss))
            .on_action(cx.listener(Self::on_confirm))
            .on_action(cx.listener(|app, _: &ToolJson, _, cx| {
                app.on_tool_shortcut(ToolId::Json, cx)
            }))
            .on_action(cx.listener(|app, _: &ToolBase64, _, cx| {
                app.on_tool_shortcut(ToolId::Base64, cx)
            }))
            .on_action(cx.listener(|app, _: &ToolTimestamp, _, cx| {
                app.on_tool_shortcut(ToolId::Timestamp, cx)
            }))
            .on_action(cx.listener(|app, _: &ToolUuid, _, cx| {
                app.on_tool_shortcut(ToolId::Uuid, cx)
            }))
            .on_action(cx.listener(|app, _: &ToolHash, _, cx| {
                app.on_tool_shortcut(ToolId::Hash, cx)
            }))
            .on_action(cx.listener(|app, _: &ToolHosts, _, cx| {
                app.on_tool_shortcut(ToolId::Hosts, cx)
            }))
            .on_action(cx.listener(|app, _: &ToolPorts, _, cx| {
                app.on_tool_shortcut(ToolId::Ports, cx)
            }))
            .on_action(cx.listener(|_: &mut DevBoxApp, _: &Quit, _, cx| cx.quit()))
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.render_topbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child({
                        if matches!(self.tool, ToolId::Diff | ToolId::Json) {
                            // The diff page is a fixed four-pane workspace
                            // that fills the area instead of scrolling.
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .p(px(14.0))
                                .child(self.render_page(cx))
                                .into_any_element()
                        } else {
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .id("app_content_scroll")
                                .overflow_y_scroll()
                                .child(div().p(px(22.0)).child(self.render_page(cx)))
                                .into_any_element()
                        }
                    }),
            )
            .child(self.render_status())
            .children(if has_palette {
                Some(self.render_palette(cx))
            } else {
                None
            })
            .children(if has_confirm {
                Some(self.render_confirm(cx))
            } else {
                None
            })
            .children(if has_hidden {
                Some(self.render_hidden_panel(cx))
            } else {
                None
            })
            .children(if keys_generate_modal {
                Some(self.render_ssh_keys_generate_modal(cx))
            } else {
                None
            })
            .children(if keys_public_modal {
                Some(self.render_ssh_keys_public_modal(cx))
            } else {
                None
            })
            .children(if credentials_modal {
                Some(self.render_ssh_credentials_form_modal(cx))
            } else {
                None
            })
            .children(if forward_modal {
                Some(self.render_ssh_forward_form_modal(cx))
            } else {
                None
            })
            .children(if proxy_modal {
                Some(self.render_api_proxy_form_modal(cx))
            } else {
                None
            })
            .children(if hosts_modal {
                Some(self.render_hosts_form_modal(cx))
            } else {
                None
            })
            .children(if has_toast {
                Some(self.render_toast())
            } else {
                None
            })
    }
}

/// Small helper so pages can reuse the app's toast/copy helpers.
pub fn action_button(
    theme: &Theme,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    cx: &mut Context<DevBoxApp>,
    handler: impl Fn(&mut DevBoxApp, &mut Context<DevBoxApp>) + 'static,
) -> impl IntoElement {
    button(theme, label, kind).id("app_auto2_15")
.on_click(cx.listener(
        move |app: &mut DevBoxApp, _: &ClickEvent, _, cx: &mut Context<DevBoxApp>| {
            handler(app, cx)
        },
    ))
}

pub fn status_badge(theme: &Theme, running: bool, label: impl Into<SharedString>) -> impl IntoElement {
    let color = if running { theme.ok } else { theme.faint };
    badge(theme, label, color)
}

pub fn mini_switch(theme: &Theme, on: bool) -> impl IntoElement {
    switch(theme, on)
}
