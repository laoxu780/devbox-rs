use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};

use crate::hosts::HostsProfile;
use crate::ui::app::{ConfirmAction, ConfirmState, DevBoxApp, LIME};
use crate::ui::pages::modal_shell;
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{
    ButtonKind, badge, button, card, empty_state, labeled, page_header, section_header, status_line,
};

pub struct HostsForm {
    pub id: String,
    pub name: Entity<TextInput>,
    pub entries: Entity<TextInput>,
}

impl HostsForm {
    fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            id: String::new(),
            name: cx.new(|cx| TextInput::single(cx, "开发环境")),
            entries: cx.new(|cx| {
                TextInput::multi_with(
                    cx,
                    "每行一条：IP 域名 [域名…]",
                    "127.0.0.1 dev.local\n10.0.0.8 api.dev.local",
                )
            }),
        }
    }

    fn reset(&mut self, cx: &mut Context<DevBoxApp>) {
        self.id.clear();
        self.name.update(cx, |input, cx| input.clear(cx));
        self.entries.update(cx, |input, cx| {
            input.set_text("127.0.0.1 dev.local\n10.0.0.8 api.dev.local", cx)
        });
    }
}

pub struct HostsState {
    pub form: HostsForm,
    pub show_form: bool,
    pub preview: String,
    pub notice: String,
    pub notice_error: bool,
}

impl HostsState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            form: HostsForm::new(cx),
            show_form: false,
            preview: String::new(),
            notice: String::new(),
            notice_error: false,
        }
    }
}

impl DevBoxApp {
    pub fn render_hosts(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = std::cell::Cell::new(0);
        let theme = self.theme;
        let profiles = self.services.hosts.list();
        let notice = self.pages.hosts.notice.clone();
        let notice_error = self.pages.hosts.notice_error;
        let preview = self.pages.hosts.preview.clone();

        let mut list = div().flex().flex_col().gap(px(6.0));
        if profiles.is_empty() {
            list = list.child(card(&theme).child(empty_state(
                &theme,
                "还没有 hosts 环境 · 点击「新增环境」创建一套域名映射",
            )));
        }
        for profile in &profiles {
            let id_apply = profile.id.clone();
            let id_preview = profile.id.clone();
            let id_edit = profile.id.clone();
            let id_delete = profile.id.clone();
            let name_delete = profile.name.clone();
            let count = profile.entries.len();
            let status = if profile.applied {
                badge(&theme, "已生效", theme.ok)
            } else if profile.active {
                badge(&theme, "已选中", theme.accent)
            } else {
                badge(&theme, "未应用", theme.faint)
            };

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
                            .child("H"),
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
                                    .child(status),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme.faint())
                                    .child(format!("{count} 条映射")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                button(&theme, "应用", ButtonKind::Primary).id(crate::ui::loop_id("hosts_apply", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                                    match app.services.hosts.apply(&id_apply) {
                                        Ok(message) => {
                                            app.pages.hosts.preview =
                                                app.services.hosts.preview(&id_apply).unwrap_or_default();
                                            app.notify(message, false, cx);
                                        }
                                        Err(error) => app.notify(error, true, cx),
                                    }
                                })),
                            )
                            .child(
                                button(&theme, "预览", ButtonKind::Subtle).id(crate::ui::loop_id("hosts_preview", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                                    app.pages.hosts.preview =
                                        app.services.hosts.preview(&id_preview).unwrap_or_default();
                                    cx.notify();
                                })),
                            )
                            .child(
                                button(&theme, "编辑", ButtonKind::Subtle).id(crate::ui::loop_id("hosts_edit", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                                    let Some(profile) = app.services.hosts.profile(&id_edit) else {
                                        return;
                                    };
                                    app.pages.hosts.form.id = profile.id;
                                    app.pages.hosts.form.name.update(cx, |input, cx| {
                                        input.set_text(profile.name, cx)
                                    });
                                    let entries = profile
                                        .entries
                                        .iter()
                                        .map(|entry| {
                                            if entry.ip.is_empty() {
                                                entry.hosts.clone()
                                            } else {
                                                format!("{}\t{}", entry.ip, entry.hosts)
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    app.pages.hosts.form.entries.update(cx, |input, cx| {
                                        input.set_text(entries, cx)
                                    });
                                    app.pages.hosts.show_form = true;
                                    cx.notify();
                                })),
                            )
                            .child(
                                button(&theme, "删除", ButtonKind::Danger).id(crate::ui::loop_id("hosts_delete", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                                    app.confirm = Some(ConfirmState {
                                        title: "删除 hosts 环境".into(),
                                        message: format!(
                                            "确定删除环境「{}」？已应用的系统 hosts 不会自动还原。",
                                            name_delete
                                        ),
                                        action: ConfirmAction::DeleteHostsProfile(id_delete.clone()),
                                    });
                                    cx.notify();
                                })),
                            ),
                    ),
            );
        }

        card(&theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_header(&theme, "H", LIME, "Hosts 环境", "一键切换本机域名解析环境"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                button(&theme, "清除 DevBox 配置", ButtonKind::Ghost)
                                    .id("hosts_clear")
                                    .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.confirm = Some(ConfirmState {
                                            title: "清除 hosts 配置".into(),
                                            message: "移除系统 hosts 中由 DevBox 管理的配置块，保留其他手动条目。".into(),
                                            action: ConfirmAction::ClearHosts,
                                        });
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button(&theme, "新增环境", ButtonKind::Primary)
                                    .debug_selector(|| "hosts_add_button".into())
                                    .id("hosts_add")
                                    .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.hosts.form.reset(cx);
                                        app.pages.hosts.show_form = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(section_header(
                &theme,
                "环境列表",
                "应用时需要 macOS 管理员授权；只会替换 DevBox 管理的配置块",
            ))
            .child(list)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.muted())
                            .child("当前 hosts 预览"),
                    )
                    .child(
                        div()
                            .max_h(px(180.0))
                            .p(px(10.0))
                            .bg(theme.inset())
                            .border_1()
                            .border_color(theme.line())
                            .rounded(px(8.0))
                            .font_family("Menlo")
                            .text_size(px(10.5))
                            .text_color(theme.text())
                            .id("hosts_preview_scroll")
                            .overflow_scroll()
                            .child(if preview.is_empty() {
                                "点击环境的「预览」或「应用」后显示".to_string()
                            } else {
                                preview
                            }),
                    ),
            )
            .child(status_line(&theme, if notice.is_empty() { "本地保存 · 切换需要管理员权限".into() } else { notice.clone() }, notice_error))
    }

    pub fn render_hosts_form_modal(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let dialog = card(&theme)
            .w(px(560.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_header(&theme, "hosts 环境", "每行一条：IP 域名，支持多个域名和 # 注释"))
                    .child(
                        button(&theme, "✕", ButtonKind::Subtle)
                            .debug_selector(|| "hosts_form_close".into())
                            .id("hosts_form_close")
                            .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.pages.hosts.show_form = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(labeled(
                &theme,
                "环境名称",
                crate::ui::pages::text_field(&theme, &self.pages.hosts.form.name, 34.0),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.muted())
                            .child("域名映射"),
                    )
                    .child(crate::ui::pages::text_area(&theme, &self.pages.hosts.form.entries, 160.0)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        button(&theme, "取消", ButtonKind::Ghost)
                            .id("hosts_form_cancel")
                            .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                app.pages.hosts.show_form = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        button(&theme, "保存", ButtonKind::Primary)
                            .debug_selector(|| "hosts_form_save".into())
                            .id("hosts_form_save")
                            .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                let entries = app.pages.hosts.form.entries.read(cx).text().to_string();
                                let profile = HostsProfile {
                                    id: app.pages.hosts.form.id.clone(),
                                    name: app.pages.hosts.form.name.read(cx).text().to_string(),
                                    entries: crate::hosts::parse_entries(&entries),
                                    position: 0,
                                };
                                match app.services.hosts.save_profile(profile) {
                                    Ok(saved) => {
                                        app.pages.hosts.show_form = false;
                                        app.pages.hosts.notice = format!("环境「{}」已保存", saved.name);
                                        app.pages.hosts.notice_error = false;
                                        app.notify("hosts 环境已保存", false, cx);
                                    }
                                    Err(error) => {
                                        app.pages.hosts.notice = error.clone();
                                        app.pages.hosts.notice_error = true;
                                        app.notify(error, true, cx);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            );

        modal_shell(
            560.0,
            "hosts_form_modal",
            cx.listener(|app, _: &gpui::MouseDownEvent, _, cx| {
                app.pages.hosts.show_form = false;
                cx.notify();
            }),
            dialog,
        )
    }
}
