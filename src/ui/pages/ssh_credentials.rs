use std::process::Command;

use gpui::{ClickEvent, Context, Entity, IntoElement, MouseDownEvent, div, prelude::*, px};

use crate::ssh::{SshProfile, SshProfileView};
use crate::theme::Theme;
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
    pub show_guide: bool,
    pub guide_os: String,
    pub notice: String,
    pub notice_error: bool,
}

impl SshCredentialsState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            form: ProfileForm::new(cx),
            show_form: false,
            show_guide: false,
            guide_os: "linux".into(),
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

struct GuideStep {
    title: &'static str,
    detail: &'static str,
    command: Option<&'static str>,
}

const LINUX_GUIDE: &[GuideStep] = &[
    GuideStep {
        title: "本机生成密钥（已有可跳过）",
        detail: "一路回车即可。生成后得到私钥 id_ed25519 和公钥 id_ed25519.pub。",
        command: Some("ssh-keygen -t ed25519 -C \"devbox\""),
    },
    GuideStep {
        title: "安装公钥 · 方式 A（推荐）",
        detail: "用 ssh-copy-id 自动安装，执行时会要求输入一次服务器密码。完成即可免密登录，无需再做步骤 3。",
        command: Some("ssh-copy-id -i ~/.ssh/id_ed25519.pub 用户名@服务器地址"),
    },
    GuideStep {
        title: "安装公钥 · 方式 B（备选）",
        detail: "与步骤 2 二选一：本机没有 ssh-copy-id 时才做这一步。把 .pub 内容追加到服务器对应用户的 ~/.ssh/authorized_keys，一行一个公钥，并确保权限正确。",
        command: Some("echo 'ssh-ed25519 AAAA...' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys"),
    },
    GuideStep {
        title: "回到 DevBox 填写凭证",
        detail: "认证方式选「私钥文件」，私钥路径填本机私钥 ~/.ssh/id_ed25519（不是 .pub），填好用户名 / 地址 / 端口后点「测试」。",
        command: None,
    },
];

const WINDOWS_GUIDE: &[GuideStep] = &[
    GuideStep {
        title: "安装 OpenSSH Server",
        detail: "以管理员身份打开 PowerShell 执行；已安装可跳过。",
        command: Some("Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0"),
    },
    GuideStep {
        title: "启动服务并设置开机自启",
        detail: "首次配置完成后再执行 Restart-Service sshd 使配置生效。",
        command: Some("Set-Service -Name sshd -StartupType Automatic; Start-Service sshd"),
    },
    GuideStep {
        title: "放行防火墙 22 端口",
        detail: "允许外部机器连接 Windows 的 SSH 服务。",
        command: Some("New-NetFirewallRule -Name sshd -DisplayName 'OpenSSH Server (sshd)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22"),
    },
    GuideStep {
        title: "安装公钥（普通账户）",
        detail: "把 .pub 内容写入 C:\\Users\\<用户名>\\.ssh\\authorized_keys，一行一个公钥。",
        command: Some("mkdir $env:USERPROFILE\\.ssh; notepad $env:USERPROFILE\\.ssh\\authorized_keys"),
    },
    GuideStep {
        title: "安装公钥（管理员账户）",
        detail: "管理员账户默认读取 administrators_authorized_keys，不是用户目录下的 authorized_keys。",
        command: Some("notepad C:\\ProgramData\\ssh\\administrators_authorized_keys"),
    },
    GuideStep {
        title: "修正权限并重启服务",
        detail: "该文件只允许 SYSTEM 与 Administrators 访问，否则 sshd 会拒绝使用该公钥。",
        command: Some("icacls C:\\ProgramData\\ssh\\administrators_authorized_keys /inheritance:r /grant SYSTEM:F /grant *S-1-5-32-544:F; Restart-Service sshd"),
    },
    GuideStep {
        title: "回到 DevBox 填写凭证",
        detail: "确认 sshd_config 中 PubkeyAuthentication yes。认证方式选「私钥文件」，用户名填 Windows 账户名（域账户用 DOMAIN\\user），点「测试」。",
        command: None,
    },
];

fn render_guide_step(
    theme: &Theme,
    step: &GuideStep,
    index: usize,
    seq: &std::cell::Cell<usize>,
    cx: &mut Context<DevBoxApp>,
) -> impl IntoElement {
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text())
                .child(format!("{}. {}", index + 1, step.title)),
        )
        .child(
            div()
                .text_size(px(10.5))
                .text_color(theme.muted())
                .child(step.detail),
        );
    if let Some(command) = step.command {
        let command_text = command.to_string();
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .p(px(7.0))
                .bg(theme.inset())
                .border_1()
                .border_color(theme.line())
                .rounded(px(6.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family("Menlo")
                        .text_size(px(10.0))
                        .text_color(theme.text())
                        .child(command_text.clone()),
                )
                .child(
                    button(theme, "复制", ButtonKind::Ghost)
                        .id(crate::ui::loop_id("ssh_credentials_guide_copy", seq))
                        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                            app.copy(command_text.clone(), cx);
                        })),
                ),
        );
    }
    div()
        .flex()
        .gap(px(9.0))
        .py(px(7.0))
        .border_b_1()
        .border_color(theme.line())
        .child(
            div()
                .w(px(18.0))
                .text_size(px(10.0))
                .text_color(theme.faint())
                .child(format!("{:02}", index + 1)),
        )
        .child(body)
}

impl DevBoxApp {
    pub fn render_ssh_credentials(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let profiles = self.services.ssh.list_profiles();
        let notice = self.pages.ssh_credentials.notice.clone();
        let notice_error = self.pages.ssh_credentials.notice_error;
        let show_guide = self.pages.ssh_credentials.show_guide;

        let guide_toggle = button(
            &theme,
            if show_guide { "收起指南" } else { "公钥配置指南" },
            ButtonKind::Ghost,
        )
        .id("ssh_credentials_guide_toggle")
        .debug_selector(|| "ssh_credentials_guide_toggle".into())
        .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
            app.pages.ssh_credentials.show_guide = !app.pages.ssh_credentials.show_guide;
            cx.notify();
        }));

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
                    panel_actions(&theme)
                        .child(guide_toggle)
                        .child(
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
                        .id("ssh_credentials_add_button")
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

        if show_guide {
            let guide_seq = std::cell::Cell::new(0);
            let guide_os = self.pages.ssh_credentials.guide_os.clone();
            let mut os_switch = segmented(&theme);
            for (value, label, selector) in [
                ("linux", "Linux 服务器", "ssh_credentials_guide_linux"),
                ("windows", "Windows 服务器", "ssh_credentials_guide_windows"),
            ] {
                os_switch = os_switch.child(
                    chip(&theme, label, guide_os == value)
                        .id(selector)
                        .debug_selector(move || selector.into())
                        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                            app.pages.ssh_credentials.guide_os = value.to_string();
                            cx.notify();
                        })),
                );
            }
            let steps = if guide_os == "windows" {
                WINDOWS_GUIDE
            } else {
                LINUX_GUIDE
            };
            let mut guide = card(&theme)
                .child(section_header(
                    &theme,
                    "公钥配置指南",
                    "在服务器装好公钥后，回到本页选择「私钥文件」认证并点「测试」",
                ))
                .child(os_switch);
            for (index, step) in steps.iter().enumerate() {
                if guide_os == "linux" && index == 1 {
                    guide = guide.child(
                        div()
                            .pl(px(27.0))
                            .pt(px(6.0))
                            .pb(px(2.0))
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.accent())
                            .child("▼ 步骤 2 与步骤 3 二选一，完成其中任意一个即可"),
                    );
                }
                guide = guide.child(render_guide_step(&theme, step, index, &guide_seq, cx));
            }
            page = page.child(guide.debug_selector(|| "ssh_credentials_guide".into()));
        }

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
