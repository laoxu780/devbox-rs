use gpui::{ClickEvent, Context, Entity, IntoElement, MouseDownEvent, div, prelude::*, px};

use crate::ssh::{SshForwardRule, SshForwardStatus};
use crate::ui::app::{CYAN, ConfirmAction, ConfirmState, DevBoxApp, ToolId};
use crate::ui::pages::{ footer_line, panel_actions, segmented, text_field, tool_page, chip };
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ ButtonKind, badge, button, card, checkbox_row, dot, empty_state, labeled, page_header, section_header, status_line, switch };

pub struct ForwardForm {
    pub id: String,
    pub name: Entity<TextInput>,
    pub local_host: Entity<TextInput>,
    pub local_port: Entity<TextInput>,
    pub remote_host: Entity<TextInput>,
    pub remote_port: Entity<TextInput>,
    pub profile_id: String,
    pub mode: String,
    pub system_proxy: bool,
}

impl ForwardForm {
    fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            id: String::new(),
            name: cx.new(|cx| TextInput::single(cx, "本地 MySQL")),
            local_host: cx.new(|cx| TextInput::single_with(cx, "127.0.0.1", "127.0.0.1")),
            local_port: cx.new(|cx| TextInput::single_with(cx, "0", "0")),
            remote_host: cx.new(|cx| TextInput::single_with(cx, "127.0.0.1", "127.0.0.1")),
            remote_port: cx.new(|cx| TextInput::single_with(cx, "80", "80")),
            profile_id: String::new(),
            mode: "local".into(),
            system_proxy: false,
        }
    }

    fn reset(&mut self, profile_id: String, cx: &mut Context<DevBoxApp>) {
        self.id.clear();
        self.name.update(cx, |input, cx| input.clear(cx));
        self.local_host
            .update(cx, |input, cx| input.set_text("127.0.0.1", cx));
        self.local_port
            .update(cx, |input, cx| input.set_text("0", cx));
        self.remote_host
            .update(cx, |input, cx| input.set_text("127.0.0.1", cx));
        self.remote_port
            .update(cx, |input, cx| input.set_text("80", cx));
        self.profile_id = profile_id;
        self.mode = "local".into();
        self.system_proxy = false;
    }

    fn load(&mut self, forward: &SshForwardStatus, cx: &mut Context<DevBoxApp>) {
        self.id = forward.id.clone();
        self.profile_id = forward.profile_id.clone();
        self.name
            .update(cx, |input, cx| input.set_text(forward.name.clone(), cx));
        self.local_host.update(cx, |input, cx| {
            input.set_text(
                if forward.local_host.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    forward.local_host.clone()
                },
                cx,
            )
        });
        self.local_port
            .update(cx, |input, cx| input.set_text(forward.local_port.to_string(), cx));
        self.remote_host.update(cx, |input, cx| {
            input.set_text(
                if forward.remote_host.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    forward.remote_host.clone()
                },
                cx,
            )
        });
        self.remote_port
            .update(cx, |input, cx| input.set_text(forward.remote_port.to_string(), cx));
        self.mode = match forward.mode.as_str() {
            "socks" => "socks".into(),
            "http" => "http".into(),
            _ => "local".into(),
        };
        self.system_proxy = forward.system_proxy;
    }
}

pub struct SshForwardState {
    pub form: ForwardForm,
    pub show_form: bool,
    pub notice: String,
    pub notice_error: bool,
}

impl SshForwardState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            form: ForwardForm::new(cx),
            show_form: false,
            notice: String::new(),
            notice_error: false,
        }
    }
}

fn mode_label(mode: &str) -> &'static str {
    match mode {
        "socks" => "SOCKS5",
        "http" => "HTTP",
        _ => "端口转发",
    }
}

fn default_proxy_port(mode: &str) -> &'static str {
    match mode {
        "socks" => "1080",
        "http" => "8080",
        _ => "0",
    }
}

impl DevBoxApp {
    pub fn render_ssh_forward(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let profiles = self.services.ssh.list_profiles();
        let forwards = self.services.ssh.list_forwards();
        let notice = self.pages.ssh_forward.notice.clone();
        let notice_error = self.pages.ssh_forward.notice_error;

        let mut page = tool_page(&theme).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(page_header(
                    &theme,
                    "→",
                    CYAN,
                    "SSH 端口转发",
                    "将服务器内网端口安全映射到本地",
                ))
                .child(
                    panel_actions(&theme).child(
                        button(
                            &theme,
                            if !self.pages.ssh_forward.form.id.is_empty() {
                                "编辑转发"
                            } else {
                                "新增转发"
                            },
                            ButtonKind::Primary,
                        )
                        .debug_selector(|| "ssh_forward_add_button".into())
                        .id("ssh_forward_auto_1")

                        .id("ssh_forward_auto2_1")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                            if app.pages.ssh_forward.show_form {
                                app.pages.ssh_forward.show_form = false;
                            } else {
                                let first = app
                                    .services
                                    .ssh
                                    .list_profiles()
                                    .first()
                                    .map(|profile| profile.id.clone())
                                    .unwrap_or_default();
                                app.pages.ssh_forward.form.reset(first, cx);
                                app.pages.ssh_forward.show_form = true;
                                app.pages.ssh_forward.notice.clear();
                                app.pages.ssh_forward.notice_error = false;
                            }
                            cx.notify();
                        })),
                    ),
                ),
        );

        if profiles.is_empty() {
            page = page.child(
                card(&theme)
                    .child(empty_state(&theme, "还没有可用的 SSH 凭证"))
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .child(
                                button(&theme, "前往 SSH 凭证", ButtonKind::Primary).id("ssh_forward_auto2_2")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.select_tool(ToolId::SshCredentials, cx)
                                    }),
                                ),
                            ),
                    ),
            );
        }

        // -- rules -----------------------------------------------------------
        let heading = div()
            .flex()
            .items_center()
            .justify_between()
            .pt(px(2.0))
            .text_size(px(11.0))
            .text_color(theme.muted())
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("转发规则"),
            )
            .child(format!("{} 条 · 重启后自动恢复", forwards.len()));
        page = page.child(heading);

        let mut list = div().flex().flex_col().gap(px(6.0));
        if forwards.is_empty() {
            list = list.child(card(&theme).child(empty_state(
                &theme,
                "暂无转发规则，点击右上角「新增转发」创建",
            )));
        }
        for forward in &forwards {
            let off = forward.status == "stopped";
            let running = forward.status == "running";
            let is_proxy = forward.mode == "socks" || forward.mode == "http";
            let state_text = if off {
                "已关闭".to_string()
            } else if running {
                if forward.reconnects > 0 {
                    format!("运行中 · 已重连 {} 次", forward.reconnects)
                } else {
                    "运行中".into()
                }
            } else if forward.status == "connecting" {
                "连接中…".into()
            } else if forward.status == "reconnecting" {
                format!("重连中 · {}", forward.error)
            } else if forward.error.is_empty() {
                forward.status.clone()
            } else {
                forward.error.clone()
            };
            let dot_color = if running || forward.status == "connecting" {
                theme.ok
            } else if forward.status == "reconnecting" {
                0xc27a2c
            } else if off {
                theme.faint
            } else {
                theme.danger
            };

            let id_toggle = forward.id.clone();
            let id_delete = forward.id.clone();
            let name_delete = forward.name.clone();
            let forward_for_edit = forward.clone();

            let mut title = div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text())
                        .child(forward.name.clone()),
                );
            if is_proxy {
                title = title.child(badge(
                    &theme,
                    mode_label(&forward.mode),
                    theme.accent,
                ));
                if forward.system_proxy && !off {
                    title = title.child(badge(&theme, "系统代理", theme.ok));
                }
            }

            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .p(px(11.0))
                    .bg(theme.card2())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(10.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(22.0))
                            .child(dot(dot_color)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .flex_1()
                            .min_w_0()
                            .child(title)
                            .child(
                                div()
                                    .font_family("Menlo")
                                    .text_size(px(10.5))
                                    .text_color(theme.muted())
                                    .child(if is_proxy {
                                        format!(
                                            "{} → 经 SSH 动态出口",
                                            forward.local_address
                                        )
                                    } else {
                                        format!(
                                            "{} → {}",
                                            forward.local_address, forward.remote_address
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme.faint())
                                    .child(format!(
                                        "{} 个连接 · {}",
                                        forward.connections, state_text
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                switch(&theme, !off).debug_selector(|| "ssh_switch".into()).id(crate::ui::loop_id("ssh_forward_auto2_8", &seq))
.on_click(cx.listener(
                                    move |app, _: &ClickEvent, _, cx| {
                                        match app
                                            .services
                                            .ssh
                                            .toggle_forward(&id_toggle, off)
                                        {
                                            Ok(message) => app.notify(message, false, cx),
                                            Err(error) => app.notify(error, true, cx),
                                        }
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "编辑", ButtonKind::Subtle).id(crate::ui::loop_id("ssh_forward_auto2_9", &seq))
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.pages
                                            .ssh_forward
                                            .form
                                            .load(&forward_for_edit, cx);
                                        app.pages.ssh_forward.show_form = true;
                                        app.pages.ssh_forward.notice.clear();
                                        app.pages.ssh_forward.notice_error = false;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "删除", ButtonKind::Danger).id(crate::ui::loop_id("ssh_forward_auto2_10", &seq))
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.confirm = Some(ConfirmState {
                                            title: "删除转发规则".into(),
                                            message: format!(
                                                "确定删除「{}」？正在运行的隧道会被关闭。",
                                                name_delete
                                            ),
                                            action: ConfirmAction::DeleteForward(
                                                id_delete.clone(),
                                            ),
                                        });
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            );
        }

        page = page.child(list);

        page.child(
            footer_line(&theme)
                .child(if notice.is_empty() {
                    status_line(
                        &theme,
                        "规则保存在本机 · 断线自动重连 · 关闭转发会还原系统代理",
                        false,
                    )
                } else {
                    status_line(&theme, notice, notice_error)
                })
                .child(div().child(format!("{} 条规则", forwards.len()))),
        )
    }
    /// Modal for adding / editing a forward rule.
    pub fn render_ssh_forward_form_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let seq = std::cell::Cell::new(0);
        let profiles = self.services.ssh.list_profiles();
        let mode = self.pages.ssh_forward.form.mode.clone();
        let system_proxy = self.pages.ssh_forward.form.system_proxy;
        let profile_id = self.pages.ssh_forward.form.profile_id.clone();
        let editing = !self.pages.ssh_forward.form.id.is_empty();

            let mut mode_switch = segmented(&theme);
            for (value, label) in [("local", "端口转发"), ("socks", "SOCKS5 代理"), ("http", "HTTP 代理")]
                .into_iter()
            {
                mode_switch = mode_switch.child(
                    chip(&theme, label, mode == value)
                        .id(crate::ui::loop_id("ssh_forward_mode", &seq))
                        .debug_selector(move || format!("ssh_forward_mode_{value}").into())
                        .on_click(cx.listener(
                        move |app, _: &ClickEvent, _, cx| {
                            let form = &mut app.pages.ssh_forward.form;
                            form.mode = value.to_string();
                            let current = form
                                .local_port
                                .read(cx)
                                .text()
                                .trim()
                                .to_string();
                            let is_default = current.is_empty()
                                || current == "0"
                                || current == "1080"
                                || current == "8080";
                            if value != "local" && is_default {
                                let next = default_proxy_port(value);
                                form.local_port
                                    .update(cx, |input, cx| input.set_text(next, cx));
                            }
                            cx.notify();
                        },
                    )),
                );
            }

            let mut profile_chips = div().flex().flex_wrap().gap(px(5.0));
            for profile in &profiles {
                let id = profile.id.clone();
                let label = format!("{} · {}", profile.name, profile.host);
                profile_chips = profile_chips.child(
                    chip(&theme, label, profile_id == id).id(crate::ui::loop_id("ssh_forward_auto2_4", &seq))
.on_click(cx.listener(
                        move |app, _: &ClickEvent, _, cx| {
                            app.pages.ssh_forward.form.profile_id = id.clone();
                            cx.notify();
                        },
                    )),
                );
            }

            let form = &self.pages.ssh_forward.form;
            let mut body = card(&theme)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(section_header(
                            &theme,
                            if editing { "编辑转发规则" } else { "新增转发" },
                            "隧道断线后自动重连，规则重启应用后恢复",
                        ))
                        .child(
                            button(&theme, "✕", ButtonKind::Subtle)
                                .debug_selector(|| "ssh_forward_dialog_close".into())
                                .id("ssh_forward_dialog_close")
                                .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                    app.pages.ssh_forward.show_form = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(mode_switch)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.muted())
                                .child("使用凭证"),
                        )
                        .child(profile_chips),
                )
                .child(labeled(
                    &theme,
                    "规则名称",
                    text_field(&theme, &form.name, 34.0),
                ));

            if mode == "local" {
                body = body
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(labeled(
                                        &theme,
                                        "本地地址",
                                        text_field(&theme, &form.local_host, 34.0),
                                    )),
                            )
                            .child(div().pb(px(8.0)).text_color(theme.faint()).child(":"))
                            .child(
                                div()
                                    .w(px(120.0))
                                    .child(labeled(
                                        &theme,
                                        "本地端口（0 随机）",
                                        text_field(&theme, &form.local_port, 34.0),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .text_color(theme.faint())
                            .child("⇅"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(labeled(
                                        &theme,
                                        "远端地址",
                                        text_field(&theme, &form.remote_host, 34.0),
                                    )),
                            )
                            .child(div().pb(px(8.0)).text_color(theme.faint()).child(":"))
                            .child(
                                div()
                                    .w(px(120.0))
                                    .child(labeled(
                                        &theme,
                                        "远端端口",
                                        text_field(&theme, &form.remote_port, 34.0),
                                    )),
                            ),
                    );
            } else {
                body = body
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(labeled(
                                        &theme,
                                        "本地监听地址",
                                        text_field(&theme, &form.local_host, 34.0),
                                    )),
                            )
                            .child(div().pb(px(8.0)).text_color(theme.faint()).child(":"))
                            .child(
                                div()
                                    .w(px(120.0))
                                    .child(labeled(
                                        &theme,
                                        "端口（0 随机）",
                                        text_field(&theme, &form.local_port, 34.0),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint())
                            .child(if mode == "socks" {
                                "SOCKS5 服务监听本地端口，所有流量经 SSH 服务器转发（动态转发）"
                            } else {
                                "HTTP 代理监听本地端口，支持 CONNECT 隧道与普通 HTTP 转发"
                            }),
                    );
                if mode != "local" {
                    body = body.child(
                        checkbox_row(&theme, "开启转发时自动设置为系统代理", system_proxy)
                            .id("ssh_forward_auto2_5")
                            .debug_selector(|| "ssh_forward_system_proxy".into())
                            .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.pages.ssh_forward.form.system_proxy =
                                    !app.pages.ssh_forward.form.system_proxy;
                                cx.notify();
                            })),
                    );
                }
            }

            body = body.child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        button(&theme, "取消", ButtonKind::Ghost).id("ssh_forward_auto2_6")
.on_click(cx.listener(
                            |app, _: &ClickEvent, _, cx| {
                                app.pages.ssh_forward.show_form = false;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        button(
                            &theme,
                            if editing { "保存修改" } else { "创建并开启" },
                            ButtonKind::Primary,
                        )
                        .id("ssh_forward_auto_3")

                        .id("ssh_forward_auto2_7")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                            let form = &app.pages.ssh_forward.form;
                            let local_port = form
                                .local_port
                                .read(cx)
                                .text()
                                .trim()
                                .parse::<i64>()
                                .unwrap_or(0);
                            let remote_port = form
                                .remote_port
                                .read(cx)
                                .text()
                                .trim()
                                .parse::<i64>()
                                .unwrap_or(0);
                            let rule = SshForwardRule {
                                id: form.id.clone(),
                                profile_id: form.profile_id.clone(),
                                name: form.name.read(cx).text().to_string(),
                                local_host: form.local_host.read(cx).text().to_string(),
                                local_port,
                                remote_host: form.remote_host.read(cx).text().to_string(),
                                remote_port,
                                disabled: false,
                                mode: if form.mode == "local" {
                                    String::new()
                                } else {
                                    form.mode.clone()
                                },
                                system_proxy: form.system_proxy,
                                position: 0,
                            };
                            let edited = !form.id.is_empty();
                            match app.services.ssh.start_forward(rule) {
                                Ok(status) => {
                                    app.pages.ssh_forward.show_form = false;
                                    app.pages.ssh_forward.notice = format!(
                                        "{}：{} → {}",
                                        if edited {
                                            "转发规则已更新"
                                        } else {
                                            "转发已创建并开启"
                                        },
                                        status.local_address,
                                        status.remote_address
                                    );
                                    app.pages.ssh_forward.notice_error = false;
                                    app.notify(
                                        if edited {
                                            "转发规则已更新"
                                        } else {
                                            "转发已开启"
                                        },
                                        false,
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    app.pages.ssh_forward.notice = error.clone();
                                    app.pages.ssh_forward.notice_error = true;
                                    app.notify(error, true, cx);
                                }
                            }
                            cx.notify();
                        })),
                    ),
            );


        crate::ui::pages::modal_shell(
            700.0,
            "ssh_forward_dialog",
            cx.listener(|app, _: &MouseDownEvent, _, cx| {
                app.pages.ssh_forward.show_form = false;
                cx.notify();
            }),
            body,
        )
    }

}
