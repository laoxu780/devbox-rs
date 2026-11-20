use gpui::{ClickEvent, Context, Div, Entity, IntoElement, div, prelude::*, px};
use serde_json::Value;

use crate::ui::app::DevBoxApp;
use crate::ui::pages::{panel_actions, segmented, chip};
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ButtonKind, button};

pub struct JsonState {
    pub input: Entity<TextInput>,
    pub pretty: bool,
}

struct JsonResult {
    ok: bool,
    text: String,
    error: String,
    nodes: usize,
}

fn run_json(input: &str, pretty: bool) -> JsonResult {
    if input.trim().is_empty() {
        return JsonResult {
            ok: false,
            text: String::new(),
            error: String::new(),
            nodes: 0,
        };
    }
    match serde_json::from_str::<Value>(input) {
        Ok(value) => {
            let text = if pretty {
                serde_json::to_string_pretty(&value).unwrap_or_default()
            } else {
                serde_json::to_string(&value).unwrap_or_default()
            };
            let nodes = count_nodes(&value);
            JsonResult {
                ok: true,
                text,
                error: String::new(),
                nodes,
            }
        }
        Err(error) => {
            let position = format!("（第 {} 行 第 {} 列）", error.line(), error.column());
            JsonResult {
                ok: false,
                text: String::new(),
                error: format!("{error}{position}"),
                nodes: 0,
            }
        }
    }
}

fn count_nodes(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(count_nodes).sum::<usize>(),
        Value::Object(map) => 1 + map.values().map(count_nodes).sum::<usize>(),
        _ => 1,
    }
}

impl JsonState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            input: cx.new(|cx| {
                TextInput::multi_with(
                    cx,
                    "粘贴或输入 JSON…",
                    r#"{"name":"devbox","version":"1.0.0","tags":["local","private"],"nested":{"ok":true}}"#,
                )
            }),
            pretty: true,
        }
    }
}

fn pane_header(theme: &crate::theme::Theme, label: &str, detail: String) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(30.0))
        .px(px(10.0))
        .bg(theme.card2())
        .border_b_1()
        .border_color(theme.line())
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text())
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.faint())
                .child(detail),
        )
}

impl DevBoxApp {
    pub fn render_json(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = std::cell::Cell::new(0);
        let theme = self.theme;
        let input = self.pages.json.input.read(cx).text().to_string();
        let pretty = self.pages.json.pretty;
        let result = run_json(&input, pretty);
        let lines = input.lines().count().max(if input.is_empty() { 0 } else { 1 });

        let mut mode = segmented(&theme);
        for (label, active) in [("格式化", pretty), ("压缩", !pretty)] {
            let next = label == "格式化";
            mode = mode.child(
                chip(&theme, label, active).id(crate::ui::loop_id("json_auto2_1", &seq))
.on_click(cx.listener(
                    move |app, _: &ClickEvent, _, cx| {
                        app.pages.json.pretty = next;
                        cx.notify();
                    },
                )),
            );
        }

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .gap(px(10.0))
            .debug_selector(|| "json_root".into())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text())
                            .child("JSON 工具"),
                    )
                    .child(
                        panel_actions(&theme)
                            .child(mode)
                            .child(
                                button(&theme, "清空", ButtonKind::Subtle).id("json_auto2_2")
.on_click(cx.listener(
                                    |app, _: &ClickEvent, _, cx| {
                                        app.pages.json.input.update(cx, |input, cx| input.clear(cx));
                                    },
                                )),
                            )
                            .child(
                                button(&theme, "复制结果", ButtonKind::Ghost).id("json_auto2_3")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        let text =
                                            app.pages.json.input.read(cx).text().to_string();
                                        let pretty = app.pages.json.pretty;
                                        let result = run_json(&text, pretty);
                                        if result.ok {
                                            app.copy(result.text, cx);
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
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .bg(theme.card())
                            .border_1()
                            .border_color(theme.line())
                            .rounded(px(10.0))
                            .overflow_hidden()
                            .debug_selector(|| "json_input".into())
                            .child(pane_header(
                                &theme,
                                "输入 JSON",
                                format!("{lines} 行 · {} 字符", input.chars().count()),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .child(self.pages.json.input.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .bg(theme.card())
                            .border_1()
                            .border_color(theme.line())
                            .rounded(px(10.0))
                            .overflow_hidden()
                            .debug_selector(|| "json_output".into())
                            .child(pane_header(
                                &theme,
                                "输出",
                                if result.ok {
                                    format!("{} 个节点", result.nodes)
                                } else {
                                    "解析失败".into()
                                },
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .p(px(10.0))
                                    .font_family("Menlo")
                                    .text_size(px(11.5))
                                    .text_color(if result.ok {
                                        theme.text()
                                    } else {
                                        theme.faint()
                                    })
                                    .id("json_output_scroll")
                                    .overflow_scroll()
                                    .child(if result.ok {
                                        result.text.clone()
                                    } else if input.trim().is_empty() {
                                        "—".into()
                                    } else {
                                        result.error.clone()
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(10.5))
                    .text_color(theme.faint())
                    .child(if input.trim().is_empty() {
                        "等待输入"
                    } else if result.ok {
                        "JSON 有效"
                    } else {
                        "JSON 无效"
                    })
                    .child("本地处理 · 不上传"),
            )
    }
}
