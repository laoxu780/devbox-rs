use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};
use md5::{Digest, Md5};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::ui::app::{DevBoxApp, VIOLET};
use crate::ui::pages::{ footer_line, io_head, panel_actions, segmented, text_area, tool_page, chip };
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ ButtonKind, button, card, page_header, status_line };

pub const HASH_ALGORITHMS: [&str; 4] = ["MD5", "SHA-1", "SHA-256", "SHA-512"];

pub struct HashState {
    pub input: Entity<TextInput>,
    pub algorithm: usize,
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub fn digest(algorithm: usize, input: &str) -> String {
    match algorithm {
        0 => hex(&Md5::digest(input.as_bytes())),
        1 => hex(&Sha1::digest(input.as_bytes())),
        2 => hex(&Sha256::digest(input.as_bytes())),
        _ => hex(&Sha512::digest(input.as_bytes())),
    }
}

impl HashState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            input: cx.new(|cx| TextInput::multi(cx, "输入要计算摘要的文本…")),
            algorithm: 2,
        }
    }
}

impl DevBoxApp {
    pub fn render_hash(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let input = self.pages.hash.input.read(cx).text().to_string();
        let algorithm = self.pages.hash.algorithm;
        let value = if input.is_empty() {
            String::new()
        } else {
            digest(algorithm, &input)
        };
        let name = HASH_ALGORITHMS[algorithm];

        let mut switch = segmented(&theme);
        for (index, label) in HASH_ALGORITHMS.iter().enumerate() {
            switch = switch.child(chip(&theme, *label, index == algorithm).id(crate::ui::loop_id("hash_auto2_1", &seq))
.on_click(
                cx.listener(move |app, _: &ClickEvent, _, cx| {
                    app.pages.hash.algorithm = index;
                    cx.notify();
                }),
            ));
        }

        tool_page(&theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_header(
                        &theme,
                        "#",
                        VIOLET,
                        "Hash 计算",
                        "MD5 / SHA 系列，实时摘要",
                    ))
                    .child(
                        panel_actions(&theme)
                            .child(switch)
                            .child(
                                button(&theme, "清空", ButtonKind::Subtle).id("hash_auto2_2")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.pages.hash.input.update(cx, |input, cx| input.clear(cx));
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "复制摘要", ButtonKind::Ghost).id("hash_auto2_3")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        let text =
                                            app.pages.hash.input.read(cx).text().to_string();
                                        let algorithm = app.pages.hash.algorithm;
                                        if !text.is_empty() {
                                            app.copy(digest(algorithm, &text), cx);
                                        }
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(io_head(
                        &theme,
                        "输入内容",
                        format!("{} 字节", input.len()),
                    ))
                    .child(text_area(&theme, &self.pages.hash.input, 200.0)),
            )
            .child(
                card(&theme).child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(
                            div()
                                .px(px(7.0))
                                .py(px(2.0))
                                .rounded(px(5.0))
                                .bg(theme.accent_tint())
                                .text_size(px(10.0))
                                .text_color(theme.accent())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_family("Menlo")
                                .text_size(px(12.0))
                                .text_color(if value.is_empty() {
                                    theme.faint()
                                } else {
                                    theme.text()
                                })
                                .child(if value.is_empty() {
                                    if input.is_empty() {
                                        "等待输入".to_string()
                                    } else {
                                        "计算中…".to_string()
                                    }
                                } else {
                                    value.clone()
                                }),
                        )
                        .child(
                            button(&theme, "复制", ButtonKind::Subtle).id("hash_auto2_4")
.on_click(cx.listener(
                                |app, _: &ClickEvent, _, cx| {
                                    let text = app.pages.hash.input.read(cx).text().to_string();
                                    let algorithm = app.pages.hash.algorithm;
                                    if !text.is_empty() {
                                        app.copy(digest(algorithm, &text), cx);
                                    }
                                },
                            )),
                        ),
                ),
            )
            .child(
                footer_line(&theme)
                    .child(if input.is_empty() {
                        status_line(&theme, "等待输入", false)
                    } else {
                        status_line(&theme, "已更新", false)
                    })
                    .child(div().child("本地计算 · 不出网")),
            )
    }
}
