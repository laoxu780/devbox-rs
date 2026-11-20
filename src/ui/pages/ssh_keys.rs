use gpui::{ClickEvent, Context, Div, Entity, IntoElement, div, prelude::*, px};

use crate::ssh::SshKeyInfo;
use crate::ui::app::{ConfirmAction, ConfirmState, DevBoxApp, VIOLET};
use crate::ui::pages::{footer_line, panel_actions, segmented, text_field, tool_page, chip};
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ButtonKind, badge, button, card, empty_state, labeled, page_header, section_header, status_line};

pub struct SshKeysState {
    pub name: Entity<TextInput>,
    pub passphrase: Entity<TextInput>,
    pub key_type: usize,
    pub show_form: bool,
    pub public_detail: Option<(String, String)>,
    pub notice: String,
    pub notice_error: bool,
}

impl SshKeysState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            name: cx.new(|cx| {
                TextInput::single_with(cx, "id_ed25519_devbox", "id_ed25519_devbox")
            }),
            passphrase: cx.new(|cx| TextInput::single(cx, "可选，留空表示无口令")),
            key_type: 0,
            show_form: false,
            public_detail: None,
            notice: String::new(),
            notice_error: false,
        }
    }
}

fn key_type_label(index: usize) -> &'static str {
    match index {
        1 => "RSA 4096",
        _ => "Ed25519",
    }
}

fn key_row(theme: &crate::theme::Theme, key: &SshKeyInfo) -> Div {
    let mut title = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text())
                .child(key.name.clone()),
        );
    if key.source == "app" {
        title = title.child(badge(theme, "DevBox", VIOLET));
    }
    if key.encrypted {
        title = title.child(badge(theme, "已加密", 0xc27a2c));
    }
    if key.in_use {
        title = title.child(badge(theme, "使用中", theme.ok));
    }
    if !key.has_public {
        title = title.child(badge(theme, "无公钥", theme.danger));
    }

    let meta = if key.bits > 0 {
        format!("{} · {} 位 · {}", key.r#type, key.bits, key.fingerprint)
    } else {
        format!("{} · {}", key.r#type, key.fingerprint)
    };
    let sub = format!(
        "{} · {} · {}",
        crate::util::human_bytes(key.size),
        key.mod_time,
        if key.source == "app" {
            "应用数据目录"
        } else {
            "~/.ssh"
        }
    );

    div()
        .flex()
        .items_center()
        .gap(px(11.0))
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
                .size(px(30.0))
                .rounded(px(8.0))
                .bg(theme.accent_ghost())
                .text_color(theme.accent())
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child("K"),
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
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(meta),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme.faint())
                        .child(sub),
                ),
        )
}

impl DevBoxApp {
    pub fn render_ssh_keys(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = std::cell::Cell::new(0);
        let theme = self.theme;
        let keys = self.services.ssh.list_keys();
        let notice = self.pages.ssh_keys.notice.clone();
        let notice_error = self.pages.ssh_keys.notice_error;

        let mut list = div().flex().flex_col().gap(px(6.0));
        if keys.is_empty() {
            list = list.child(card(&theme).child(empty_state(
                &theme,
                "没有找到密钥 · 点击「生成密钥」创建一对新密钥",
            )));
        }
        for key in keys.iter() {
            let key_path = key.path.clone();
            let key_name = key.name.clone();
            let source = key.source.clone();
            let in_use = key.in_use;
            let mut actions = div().flex().items_center().gap(px(4.0)).child(
                button(&theme, "查看公钥", ButtonKind::Subtle).debug_selector(|| format!("ssh_view_pub_{}", key.name)).id(crate::ui::loop_id("ssh_keys_auto2_2", &seq))
.on_click(cx.listener(
                    move |app, _: &ClickEvent, _, cx| match app
                        .services
                        .ssh
                        .read_public_key(&key_path)
                    {
                        Ok(public) => {
                            app.pages.ssh_keys.public_detail =
                                Some((key_path.clone(), public));
                            app.pages.ssh_keys.notice.clear();
                            app.pages.ssh_keys.notice_error = false;
                            cx.notify();
                        }
                        Err(error) => {
                            app.pages.ssh_keys.notice = error.clone();
                            app.pages.ssh_keys.notice_error = true;
                            cx.notify();
                        }
                    },
                )),
            );
            let path_for_copy = key.path.clone();
            actions = actions.child(
                button(&theme, "复制路径", ButtonKind::Subtle).id(crate::ui::loop_id("ssh_keys_auto2_3", &seq))
.on_click(
                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.copy(path_for_copy.clone(), cx)
                    }),
                ),
            );
            if source == "app" && !in_use {
                let path_for_delete = key.path.clone();
                actions = actions.child(
                    button(&theme, "删除", ButtonKind::Danger).id(crate::ui::loop_id("ssh_keys_auto2_4", &seq))
.on_click(cx.listener(
                        move |app, _: &ClickEvent, _, cx| {
                            app.confirm = Some(ConfirmState {
                                title: "删除密钥".into(),
                                message: format!(
                                    "确定删除密钥「{}」？私钥与公钥文件都会被移除，无法恢复。",
                                    key_name
                                ),
                                action: ConfirmAction::DeleteKey(path_for_delete.clone()),
                            });
                            cx.notify();
                        },
                    )),
                );
            } else if in_use {
                actions = actions.child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme.faint())
                        .child("凭证引用中"),
                );
            }
            let mut row = key_row(&theme, key);
            row = row.child(actions);
            list = list.child(row);
        }

        let page = tool_page(&theme).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(page_header(
                    &theme,
                    "K",
                    VIOLET,
                    "SSH 密钥",
                    "查看、生成与导出本机密钥",
                ))
                .child(
                    panel_actions(&theme)
                        .child(
                            button(&theme, "刷新", ButtonKind::Subtle).id("ssh_keys_auto2_5")
.on_click(
                                cx.listener(|app, _: &ClickEvent, _, cx| {
                                    app.pages.ssh_keys.notice = "已刷新".into();
                                    app.pages.ssh_keys.notice_error = false;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            button(&theme, "生成密钥", ButtonKind::Primary)
                                .debug_selector(|| "ssh_generate_button".into())
                                .id("ssh_keys_auto_1")
                                .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                    app.pages.ssh_keys.show_form = true;
                                    cx.notify();
                                })),
                        ),
                ),
        );

        page.child(list).child(
            footer_line(&theme)
                .child(if notice.is_empty() {
                    status_line(
                        &theme,
                        "DevBox 生成的密钥保存在应用数据目录 · ~/.ssh 中的密钥只读展示",
                        false,
                    )
                } else {
                    status_line(&theme, notice, notice_error)
                })
                .child(div().child(format!("{} 个密钥", keys.len()))),
        )
    }

    /// Modal for generating a new key pair.
    pub fn render_ssh_keys_generate_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let seq = std::cell::Cell::new(0);
        let key_type = self.pages.ssh_keys.key_type;

        let mut mode = segmented(&theme);
        for (index, label) in ["Ed25519", "RSA 4096"].iter().enumerate() {
            mode = mode.child(chip(&theme, *label, key_type == index).id(crate::ui::loop_id("ssh_keys_modal_type", &seq))
.on_click(
                cx.listener(move |app, _: &ClickEvent, _, cx| {
                    app.pages.ssh_keys.key_type = index;
                    cx.notify();
                }),
            ));
        }

        let dialog = card(&theme)
            .w(px(500.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .debug_selector(|| "ssh_generate_dialog".into())
            .id("ssh_keys_generate_dialog")
            .on_mouse_down_out(cx.listener(|app, _: &gpui::MouseDownEvent, _, cx| {
                app.pages.ssh_keys.show_form = false;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_header(
                        &theme,
                        "生成密钥对",
                        "密钥保存在 DevBox 应用数据目录，不会覆盖 ~/.ssh",
                    ))
                    .child(
                        button(&theme, "✕", ButtonKind::Subtle).debug_selector(|| "ssh_generate_close".into()).id("ssh_keys_modal_close")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                            app.pages.ssh_keys.show_form = false;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().text_size(px(11.0)).text_color(theme.muted()).child("类型"))
                    .child(mode),
            )
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(labeled(
                                &theme,
                                "文件名",
                                text_field(&theme, &self.pages.ssh_keys.name, 34.0),
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(labeled(
                                &theme,
                                "口令（可选）",
                                text_field(&theme, &self.pages.ssh_keys.passphrase, 34.0),
                            )),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(theme.faint())
                    .child(format!(
                        "保存位置：{}",
                        crate::store::config_path("keys").display()
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        button(&theme, "取消", ButtonKind::Ghost).id("ssh_keys_modal_cancel")
.on_click(
                            cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.pages.ssh_keys.show_form = false;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button(&theme, "生成", ButtonKind::Primary).id("ssh_keys_modal_generate")
.on_click(
                            cx.listener(|app, _: &ClickEvent, _, cx| {
                                let name = app.pages.ssh_keys.name.read(cx).text().to_string();
                                let passphrase = app
                                    .pages
                                    .ssh_keys
                                    .passphrase
                                    .read(cx)
                                    .text()
                                    .to_string();
                                let kind = key_type_label(app.pages.ssh_keys.key_type);
                                let result = app.services.ssh.generate_key(
                                    if app.pages.ssh_keys.key_type == 1 {
                                        "rsa"
                                    } else {
                                        "ed25519"
                                    },
                                    &name,
                                    &passphrase,
                                );
                                match result {
                                    Ok(info) => {
                                        app.pages.ssh_keys.show_form = false;
                                        app.pages.ssh_keys.notice = format!(
                                            "已生成 {} 密钥：{}",
                                            kind, info.path
                                        );
                                        app.pages.ssh_keys.notice_error = false;
                                        app.pages.ssh_keys.passphrase.update(
                                            cx,
                                            |input, cx| input.clear(cx),
                                        );
                                        app.notify("密钥已生成", false, cx);
                                    }
                                    Err(error) => {
                                        app.pages.ssh_keys.notice = error.clone();
                                        app.pages.ssh_keys.notice_error = true;
                                        app.notify(error, true, cx);
                                    }
                                }
                                cx.notify();
                            }),
                        ),
                    ),
            );

        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000055))
            .child(dialog)
    }

    /// Modal showing the public key of a selected key pair.
    pub fn render_ssh_keys_public_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let (path, public) = self
            .pages
            .ssh_keys
            .public_detail
            .clone()
            .unwrap_or_default();
        let path_for_copy = path.clone();
        let public_for_copy = public.clone();

        let dialog = card(&theme)
            .w(px(620.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .debug_selector(|| "ssh_public_dialog".into())
            .id("ssh_keys_public_dialog")
            .on_mouse_down_out(cx.listener(|app, _: &gpui::MouseDownEvent, _, cx| {
                app.pages.ssh_keys.public_detail = None;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_header(&theme, "公钥内容", path.clone()))
                    .child(
                        button(&theme, "✕", ButtonKind::Subtle).debug_selector(|| "ssh_public_close".into()).id("ssh_keys_public_close")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                            app.pages.ssh_keys.public_detail = None;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .p(px(10.0))
                    .bg(theme.inset())
                    .rounded(px(8.0))
                    .font_family("Menlo")
                    .text_size(px(11.0))
                    .text_color(theme.text())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(public_for_copy.clone()),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        button(&theme, "复制路径", ButtonKind::Ghost).id("ssh_keys_public_copy_path")
.on_click(
                            cx.listener(move |app, _: &ClickEvent, _, cx| {
                                app.copy(path_for_copy.clone(), cx)
                            }),
                        ),
                    )
                    .child(
                        button(&theme, "复制公钥", ButtonKind::Primary).id("ssh_keys_public_copy_key")
.on_click(
                            cx.listener(move |app, _: &ClickEvent, _, cx| {
                                app.copy(public_for_copy.clone(), cx)
                            }),
                        ),
                    ),
            );

        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000055))
            .child(dialog)
    }
}
