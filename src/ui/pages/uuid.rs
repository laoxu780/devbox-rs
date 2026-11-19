use gpui::{ClickEvent, Context, IntoElement, div, prelude::*, px};

use crate::ui::app::{DevBoxApp, PINK};
use crate::ui::pages::{ footer_line, panel_actions, segmented, tool_page, chip };
use crate::ui::widgets::{ ButtonKind, button, card, checkbox_row, empty_state, page_header, status_line };

pub struct UuidState {
    pub count: usize,
    pub upper: bool,
    pub no_hyphen: bool,
    pub list: Vec<String>,
}

impl UuidState {
    pub fn new() -> Self {
        let mut state = Self {
            count: 5,
            upper: false,
            no_hyphen: false,
            list: Vec::new(),
        };
        state.generate();
        state
    }

    pub fn generate(&mut self) {
        self.list = (0..self.count)
            .map(|_| {
                let mut id = uuid::Uuid::new_v4().to_string();
                if self.no_hyphen {
                    id = id.replace('-', "");
                }
                if self.upper {
                    id = id.to_uppercase();
                }
                id
            })
            .collect();
    }
}

impl Default for UuidState {
    fn default() -> Self {
        Self::new()
    }
}

impl DevBoxApp {
    pub fn render_uuid(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let count = self.pages.uuid.count;
        let upper = self.pages.uuid.upper;
        let no_hyphen = self.pages.uuid.no_hyphen;
        let list = self.pages.uuid.list.clone();

        let mut count_switch = segmented(&theme);
        for option in [1usize, 5, 10, 20] {
            count_switch = count_switch.child(chip(
                &theme,
                format!("{option} 个"),
                count == option,
            ).id(crate::ui::loop_id("uuid_auto2_1", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                app.pages.uuid.count = option;
                app.pages.uuid.generate();
                cx.notify();
            })));
        }

        let mut rows = card(&theme).gap(px(0.0)).p(px(0.0));
        if list.is_empty() {
            rows = rows.child(empty_state(&theme, "点击「重新生成」"));
        }
        for (index, id) in list.iter().enumerate() {
            let value = id.clone();
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(34.0))
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(theme.line())
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.accent_ghost()))
                    .id(crate::ui::loop_id("uuid_auto_1", &seq))

                    .id(crate::ui::loop_id("uuid_auto2_2", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.copy(value.clone(), cx)
                    }))
                    .child(
                        div()
                            .w(px(26.0))
                            .font_family("Menlo")
                            .text_size(px(10.0))
                            .text_color(theme.faint())
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family("Menlo")
                            .text_size(px(12.0))
                            .text_color(theme.text())
                            .child(id.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme.faint())
                            .child("点击复制"),
                    ),
            );
        }

        tool_page(&theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_header(
                        &theme,
                        "ID",
                        PINK,
                        "UUID 生成器",
                        "UUID v4，批量生成",
                    ))
                    .child(
                        panel_actions(&theme)
                            .child(count_switch)
                            .child(
                                checkbox_row(&theme, "大写", upper).id("uuid_auto2_3")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.pages.uuid.upper = !app.pages.uuid.upper;
                                        app.pages.uuid.generate();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                checkbox_row(&theme, "无连字符", no_hyphen).id("uuid_auto2_4")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.uuid.no_hyphen = !app.pages.uuid.no_hyphen;
                                        app.pages.uuid.generate();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "清空", ButtonKind::Subtle).id("uuid_auto2_5")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.pages.uuid.list.clear();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "重新生成", ButtonKind::Primary).id("uuid_auto2_6")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.uuid.generate();
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .child(rows)
            .child(
                footer_line(&theme)
                    .child(if list.is_empty() {
                        status_line(&theme, "等待生成", false)
                    } else {
                        status_line(
                            &theme,
                            format!("{} 个 UUID v4 · 点击任意一条复制", list.len()),
                            false,
                        )
                    })
                    .child(
                        button(&theme, "复制全部", ButtonKind::Ghost).id("uuid_auto2_7")
.on_click(cx.listener(
                            |app, _: &ClickEvent, _, cx| {
                                let text = app.pages.uuid.list.join("\n");
                                app.copy(text, cx);
                            },
                        )),
                    ),
            )
    }
}
