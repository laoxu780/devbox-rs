use std::process::Command;

use gpui::{ClickEvent, Context, Entity, IntoElement, MouseDownEvent, div, prelude::*, px};

use crate::ssh::{SshProfile, SshProfileView};
use crate::ui::app::{CYAN, ConfirmAction, ConfirmState, DevBoxApp};
use crate::ui::pages::{ footer_line, panel_actions, segmented, text_field, tool_page, chip };
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ ButtonKind, badge, button, card, checkbox_row, empty_state, labeled, page_header, section_header, status_line };

pub struct ProfileForm {
    pub id: String,
    pub name: Entity<TextInput>,
    pub host: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub username: Entity<TextInput>,
    pub key_path: Entity<TextInput>,
    pub secret: Entity<TextInput>,
    pub auth_type: String,
    pub strict: bool,
}

impl ProfileForm {
    fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            id: String::new(),
            name: cx.new(|cx| TextInput::single(cx, "生产跳板机")),
            host: cx.new(|cx| TextInput::single(cx, "bastion.example.com")),
            port: cx.new(|cx| TextInput::single_with(cx, "22", "22")),
            username: cx.new(|cx| TextInput::single_with(cx, "root", "root")),
            key_path: cx.new(|cx| TextInput::single(cx, "~/.ssh/id_ed25519")),
            secret: cx.new(|cx| TextInput::single(cx, "登录密码或私钥口令")),
            auth_type: "password".into(),
            strict: false,
        }
    }

    pub fn reset(&mut self, cx: &mut Context<DevBoxApp>) {
        self.id.clear();
        self.name.update(cx, |input, cx| input.clear(cx));
        self.host.update(cx, |input, cx| input.clear(cx));
        self.port.update(cx, |input, cx| input.set_text("22", cx));
        self.username.update(cx, |input, cx| input.set_text("root", cx));
        self.key_path.update(cx, |input, cx| input.clear(cx));
        self.secret.update(cx, |input, cx| input.clear(cx));
        self.auth_type = "password".into();
        self.strict = false;
    }

    fn load(&mut self, profile: &SshProfileView, cx: &mut Context<DevBoxApp>) {
        self.id = profile.id.clone();
        self.name
            .update(cx, |input, cx| input.set_text(profile.name.clone(), cx));
        self.host
            .update(cx, |input, cx| input.set_text(profile.host.clone(), cx));
        self.port.update(cx, |input, cx| {
            input.set_text(profile.port.max(1).to_string(), cx)
        });
        self.username
            .update(cx, |input, cx| input.set_text(profile.username.clone(), cx));
        self.key_path.update(cx, |input, cx| {
            input.set_text(profile.private_key_path.clone(), cx)
        });
        self.secret.update(cx, |input, cx| input.clear(cx));
        self.auth_type = if profile.auth_type == "key" {
            "key".into()
        } else {
            "password".into()
        };
        self.strict = profile.strict_host_key_checking;
    }
}

pub struct SshCredentialsState {
    pub form: ProfileForm,
    pub show_form: bool,
    pub notice: String,
    pub notice_error: bool,
}

impl SshCredentialsState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            form: ProfileForm::new(cx),
            show_form: false,
            notice: String::new(),
            notice_error: false,
        }
    }
}

fn pick_key_file() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return crate::util::pick_file_path("选择 SSH 私钥文件", &["*"]);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose file with prompt \"选择 SSH 私钥文件\")",
            ])
            .output()
            .map_err(|error| format!("打开文件选择器失败：{error}"))?;
        if !output.status.success() {
            return Err(String::new());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

impl DevBoxApp {
    pub fn render_ssh_credentials(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let profiles = self.services.ssh.list_profiles();
        let notice = self.pages.ssh_credentials.notice.clone();
        let notice_error = self.pages.ssh_credentials.notice_error;

        let mut list = div().flex().flex_col().gap(px(6.0));
        if profiles.is_empty() {
            list = list.child(card(&theme).child(empty_state(
                &theme,
                "还没有 SSH 凭证 · 新增凭证后即可在「SSH 端口转发」中建立隧道",
            )));
        }
        for profile in &profiles {
            let auth = if profile.auth_type == "key" {
                "私钥认证"
            } else {
                "密码认证"
            };
            let has_secret = if profile.has_secret {
                " · 已保存凭证"
            } else {
                ""
            };
            let id_for_test = profile.id.clone();
            let id_for_delete = profile.id.clone();
            let name_for_delete = profile.name.clone();
            let host_for_test = profile.host.clone();
            let profile_for_edit = profile.clone();

            list = list.child(
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
                            .child("C"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text())
                                            .child(profile.name.clone()),
                                    )
                                    .child(badge(
                                        &theme,
                                        auth,
                                        if profile.auth_type == "key" {
                                            CYAN
                                        } else {
                                            0xc27a2c
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .font_family("Menlo")
                                    .text_size(px(10.5))
                                    .text_color(theme.muted())
                                    .child(format!(
                                        "{}@{}:{}",
                                        profile.username, profile.host, profile.port
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme.faint())
                                    .child(format!("{auth}{has_secret}")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                button(&theme, "测试", ButtonKind::Subtle).id(crate::ui::loop_id("ssh_credentials_auto2_1", &seq))
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        let host_for_test = host_for_test.clone();
                                        app.pages.ssh_credentials.notice =
                                            format!("正在连接 {host_for_test}…");
                                        app.pages.ssh_credentials.notice_error = false;
                                        cx.notify();
                                        let ssh = app.services.ssh.clone();
                                        let id = id_for_test.clone();
                                        cx.spawn(async move |this, cx| {
                                            let (tx, rx) = tokio::sync::oneshot::channel();
                                            std::thread::spawn(move || {
                                                let _ = tx.send(ssh.test_profile(&id));
                                            });
                                            let result = rx.await;
                                            this.update(cx, |app, cx| {
                                                match result {
                                                    Ok(Ok(())) => {
                                                        app.pages.ssh_credentials.notice =
                                                            format!("已成功连接 {host_for_test}");
                                                        app.pages.ssh_credentials.notice_error =
                                                            false;
                                                        app.notify("SSH 连接成功", false, cx);
                                                    }
                                                    Ok(Err(error)) => {
                                                        app.pages.ssh_credentials.notice =
                                                            error.clone();
                                                        app.pages.ssh_credentials.notice_error =
                                                            true;
                                                        app.notify(error, true, cx);
                                                    }
                                                    Err(_) => {}
                                                }
                                                cx.notify();
                                            })
                                            .ok();
                                        })
                                        .detach();
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "编辑", ButtonKind::Subtle).id(crate::ui::loop_id("ssh_credentials_auto2_2", &seq))
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.pages
                                            .ssh_credentials
                                            .form
                                            .load(&profile_for_edit, cx);
                                        app.pages.ssh_credentials.show_form = true;
                                        app.pages.ssh_credentials.notice.clear();
                                        app.pages.ssh_credentials.notice_error = false;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "删除", ButtonKind::Danger).id(crate::ui::loop_id("ssh_credentials_auto2_3", &seq))
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.confirm = Some(ConfirmState {
                                            title: "删除凭证".into(),
                                            message: format!(
                                                "确定删除凭证「{}」？关联的转发规则会一并移除。",
                                                name_for_delete
                                            ),
                                            action: ConfirmAction::DeleteProfile(
                                                id_for_delete.clone(),
                                            ),
                                        });
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            );
        }

        let mut page = tool_page(&theme).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(page_header(
                    &theme,
                    "C",
                    CYAN,
                    "SSH 凭证",
                    "保存服务器连接配置与登录密钥",
                ))
                .child(
                    panel_actions(&theme).child(
                        button(
                            &theme,
                            if !self.pages.ssh_credentials.form.id.is_empty() {
                                "编辑凭证"
                            } else {
                                "新增凭证"
                            },
                            ButtonKind::Primary,
                        )
                        .debug_selector(|| "ssh_credentials_add_button".into())
                        .id("ssh_credentials_auto_1")

                        .id("ssh_credentials_auto2_4")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                            if app.pages.ssh_credentials.show_form {
                                app.pages.ssh_credentials.show_form = false;
                            } else {
                                app.pages
                                    .ssh_credentials
                                    .form
                                    .reset(cx);
                                app.pages.ssh_credentials.show_form = true;
                                app.pages.ssh_credentials.notice.clear();
                                app.pages.ssh_credentials.notice_error = false;
                            }
                            cx.notify();
                        })),
                    ),
                ),
        );

        page = page.child(list);

        page.child(
            footer_line(&theme)
                .child(if notice.is_empty() {
                    status_line(
                        &theme,
                        "凭证保存在本机，转发规则重启后自动恢复",
                        false,
                    )
                } else {
                    status_line(&theme, notice, notice_error)
                })
                .child(div().child(format!("{} 个凭证", profiles.len()))),
        )
    }
    /// Modal for adding / editing an SSH credential.
    pub fn render_ssh_credentials_form_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let seq = std::cell::Cell::new(0);
        let keys = self.services.ssh.list_keys();
        let editing = !self.pages.ssh_credentials.form.id.is_empty();
        let auth_type = self.pages.ssh_credentials.form.auth_type.clone();
        let strict = self.pages.ssh_credentials.form.strict;

            let mut auth_switch = segmented(&theme);
            for (value, label) in [("password", "密码"), ("key", "私钥文件")] {
                auth_switch = auth_switch.child(chip(
                    &theme,
                    label,
                    self.pages.ssh_credentials.form.auth_type == value,
                ).id(crate::ui::loop_id("ssh_credentials_auto2_5", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                    app.pages.ssh_credentials.form.auth_type = value.to_string();
                    cx.notify();
                })));
            }

            let mut key_picker = div().flex().flex_col().gap(px(6.0));
            if !keys.is_empty() {
                let mut chips = div().flex().flex_wrap().gap(px(5.0));
                for key in keys.iter().take(12) {
                    let path = key.path.clone();
                    let label = format!(
                        "{} · {}{}",
                        key.name,
                        key.r#type,
                        if key.encrypted { " · 加密" } else { "" }
                    );
                    chips = chips.child(
                        button(&theme, label, ButtonKind::Ghost).id(crate::ui::loop_id("ssh_credentials_auto2_6", &seq))
.on_click(cx.listener(
                            move |app, _: &ClickEvent, _, cx| {
                                app.pages.ssh_credentials.form.key_path.update(
                                    cx,
                                    |input, cx| input.set_text(path.clone(), cx),
                                );
                                cx.notify();
                            },
                        )),
                    );
                }
                key_picker = key_picker.child(chips);
            }
            key_picker = key_picker.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(text_field(
                                &theme,
                                &self.pages.ssh_credentials.form.key_path,
                                34.0,
                            )),
                    )
                    .child(
                        button(&theme, "浏览…", ButtonKind::Ghost).id("ssh_credentials_auto2_7")
.on_click(cx.listener(
                            |app, _: &ClickEvent, _, cx| match pick_key_file() {
                                Ok(path) if !path.is_empty() => {
                                    app.pages.ssh_credentials.form.key_path.update(
                                        cx,
                                        |input, cx| input.set_text(path, cx),
                                    );
                                    cx.notify();
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    app.pages.ssh_credentials.notice = error.clone();
                                    app.pages.ssh_credentials.notice_error = true;
                                    app.notify(error, true, cx);
                                }
                            },
                        )),
                    ),
            );

            let form = &self.pages.ssh_credentials.form;
                let dialog = card(&theme)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(section_header(
                                &theme,
                                if editing { "编辑凭证" } else { "新增凭证" },
                                "配置保存在本机，密码只写入本地配置文件（0600）",
                            ))
                            .child(
                                button(&theme, "✕", ButtonKind::Subtle)
                                    .debug_selector(|| "ssh_credentials_dialog_close".into())
                                    .id("ssh_credentials_dialog_close")
                                    .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.ssh_credentials.show_form = false;
                                        cx.notify();
                                    })),
                            ),
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
                                        "名称",
                                        text_field(&theme, &form.name, 34.0),
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(labeled(
                                        &theme,
                                        "用户名",
                                        text_field(&theme, &form.username, 34.0),
                                    )),
                            ),
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
                                        "服务器地址",
                                        text_field(&theme, &form.host, 34.0),
                                    )),
                            )
                            .child(
                                div()
                                    .w(px(110.0))
                                    .child(labeled(
                                        &theme,
                                        "SSH 端口",
                                        text_field(&theme, &form.port, 34.0),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme.muted())
                                    .child("认证方式"),
                            )
                            .child(auth_switch),
                    )
                    .children(if auth_type == "key" {
                        Some(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(5.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme.muted())
                                        .child("私钥文件"),
                                )
                                .child(key_picker),
                        )
                    } else {
                        None
                    })
                    .child(labeled(
                        &theme,
                        if auth_type == "key" {
                            "私钥口令（可选）"
                        } else {
                            "登录密码"
                        },
                        text_field(&theme, &form.secret, 34.0),
                    ))
                    .child(
                        checkbox_row(&theme, "严格校验 ~/.ssh/known_hosts", strict).id("ssh_credentials_auto2_8")
.on_click(
                            cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.pages.ssh_credentials.form.strict =
                                    !app.pages.ssh_credentials.form.strict;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                button(&theme, "取消", ButtonKind::Ghost).id("ssh_credentials_auto2_9")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.ssh_credentials.show_form = false;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "保存凭证", ButtonKind::Primary).id("ssh_credentials_auto2_10")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        let form = &app.pages.ssh_credentials.form;
                                        let port = form
                                            .port
                                            .read(cx)
                                            .text()
                                            .trim()
                                            .parse::<i64>()
                                            .unwrap_or(22);
                                        let profile = SshProfile {
                                            id: form.id.clone(),
                                            name: form.name.read(cx).text().to_string(),
                                            host: form.host.read(cx).text().to_string(),
                                            port,
                                            username: form.username.read(cx).text().to_string(),
                                            auth_type: form.auth_type.clone(),
                                            private_key_path: form
                                                .key_path
                                                .read(cx)
                                                .text()
                                                .to_string(),
                                            strict_host_key_checking: form.strict,
                                            secret: form.secret.read(cx).text().to_string(),
                                        };
                                        match app.services.ssh.save_profile(profile) {
                                            Ok(saved) => {
                                                app.pages.ssh_credentials.show_form = false;
                                                app.pages.ssh_credentials.notice =
                                                    format!("凭证「{}」已保存", saved.name);
                                                app.pages.ssh_credentials.notice_error =
                                                    false;
                                                app.pages.ssh_credentials.form.reset(cx);
                                                app.notify("SSH 凭证已保存", false, cx);
                                            }
                                            Err(error) => {
                                                app.pages.ssh_credentials.notice =
                                                    error.clone();
                                                app.pages.ssh_credentials.notice_error = true;
                                                app.notify(error, true, cx);
                                            }
                                        }
                                        cx.notify();
                                    }),
                                ),
                            ),
                    )
            ;

        crate::ui::pages::modal_shell(
            660.0,
            "ssh_credentials_dialog",
            cx.listener(|app, _: &MouseDownEvent, _, cx| {
                app.pages.ssh_credentials.show_form = false;
                cx.notify();
            }),
            dialog,
        )
    }

}
