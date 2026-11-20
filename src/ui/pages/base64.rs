use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};

use crate::ui::app::{CYAN, DevBoxApp};
use crate::ui::pages::{ footer_line, io_head, panel_actions, segmented, text_area, tool_page, chip };
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ ButtonKind, button, checkbox_row, page_header, status_line };

pub struct Base64State {
    pub input: Entity<TextInput>,
    pub encode: bool,
    pub url_safe: bool,
}

fn encode(text: &str, url_safe: bool) -> String {
    if url_safe {
        URL_SAFE_NO_PAD.encode(text.as_bytes())
    } else {
        STANDARD.encode(text.as_bytes())
    }
}

fn decode(text: &str, url_safe: bool) -> Result<String, String> {
    let cleaned: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Ok(String::new());
    }
    let mut normalized = if url_safe || cleaned.contains(['-', '_']) {
        cleaned.replace('-', "+").replace('_', "/")
    } else {
        cleaned
    };
    while normalized.len() % 4 != 0 {
        normalized.push('=');
    }
    let bytes = STANDARD
        .decode(normalized.as_bytes())
        .map_err(|_| "不是有效的 Base64 内容".to_string())?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

impl Base64State {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            input: cx.new(|cx| TextInput::multi(cx, "输入要编码的文本或粘贴 Base64…")),
            encode: true,
            url_safe: false,
        }
    }
}

impl DevBoxApp {
    pub fn render_base64(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = std::cell::Cell::new(0);
        let theme = self.theme;
        let input = self.pages.base64.input.read(cx).text().to_string();
        let encode_mode = self.pages.base64.encode;
        let url_safe = self.pages.base64.url_safe;
        let output = if input.is_empty() {
            Ok(String::new())
        } else if encode_mode {
            Ok(encode(&input, url_safe))
        } else {
            decode(&input, url_safe)
        };
        let ok = output.is_ok();
        let output_text = output.clone().unwrap_or_default();

        let mut mode = segmented(&theme);
        for (label, active) in [("编码", encode_mode), ("解码", !encode_mode)] {
            let next = label == "编码";
            mode = mode.child(
                chip(&theme, label, active).id(crate::ui::loop_id("base64_auto2_1", &seq))
.on_click(cx.listener(
                    move |app, _: &ClickEvent, _, cx| {
                        app.pages.base64.encode = next;
                        cx.notify();
                    },
                )),
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
                        "64",
                        CYAN,
                        "Base64 编解码",
                        "UTF-8 安全的实时转换",
                    ))
                    .child(
                        panel_actions(&theme)
                            .child(mode)
                            .child(
                                checkbox_row(&theme, "URL 安全", url_safe).id("base64_auto2_2")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.pages.base64.url_safe = !app.pages.base64.url_safe;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "清空", ButtonKind::Subtle).id("base64_auto2_3")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.pages.base64.input.update(cx, |input, cx| input.clear(cx));
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "复制结果", ButtonKind::Ghost).id("base64_auto2_4")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        let text =
                                            app.pages.base64.input.read(cx).text().to_string();
                                        let encode_mode = app.pages.base64.encode;
                                        let url_safe = app.pages.base64.url_safe;
                                        let output = if encode_mode {
                                            Ok(encode(&text, url_safe))
                                        } else {
                                            decode(&text, url_safe)
                                        };
                                        if let Ok(output) = output {
                                            app.copy(output, cx);
                                        }
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(io_head(
                                &theme,
                                if encode_mode { "原文" } else { "Base64" },
                                format!("{} 字符", input.chars().count()),
                            ))
                            .child(text_area(&theme, &self.pages.base64.input, 320.0)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(io_head(
                                &theme,
                                if encode_mode { "Base64" } else { "原文" },
                                if ok {
                                    format!("{} 字符", output_text.chars().count())
                                } else {
                                    "解码失败".into()
                                },
                            ))
                            .child(
                                div()
                                    .h(px(320.0))
                                    .w_full()
                                    .p(px(10.0))
                                    .font_family("Menlo")
                                    .text_size(px(11.5))
                                    .text_color(if ok { theme.text() } else { theme.danger() })
                                    .bg(theme.card())
                                    .border_1()
                                    .border_color(if ok { theme.line() } else { theme.danger() })
                                    .rounded(px(9.0))
                                    .id("base64_auto_1")

                                    .id("base64_auto2_5")
.overflow_scroll()
                                    .child(output_text),
                            ),
                    ),
            )
            .child(
                footer_line(&theme)
                    .child(if input.is_empty() {
                        status_line(&theme, "等待输入", false)
                    } else if ok {
                        status_line(&theme, "转换成功", false)
                    } else {
                        status_line(&theme, output.err().unwrap_or_default(), true)
                    })
                    .child(div().child("UTF-8 · 本地转换")),
            )
    }
}
