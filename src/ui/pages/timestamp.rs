use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};

use crate::ui::app::{DevBoxApp, ORANGE};
use crate::ui::pages::{footer_line, panel_actions, text_field, tool_page};
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ButtonKind, button, card, icon_button, page_header, status_line};

pub struct TimestampState {
    pub input: Entity<TextInput>,
}

fn relative_time(when: DateTime<Utc>) -> String {
    let diff = Utc::now().signed_duration_since(when).num_seconds();
    let abs = diff.abs();
    if abs < 1 {
        return "刚刚".into();
    }
    let (value, unit) = if abs < 60 {
        (abs, "秒")
    } else if abs < 3600 {
        (abs / 60, "分钟")
    } else if abs < 86400 {
        (abs / 3600, "小时")
    } else {
        (abs / 86400, "天")
    };
    if diff > 0 {
        format!("{value} {unit}前")
    } else {
        format!("{value} {unit}后")
    }
}

fn fields_for(when: DateTime<Utc>) -> Vec<(String, String)> {
    let local = when.with_timezone(&Local);
    vec![
        (
            "本地时间".into(),
            local.format("%Y-%m-%d %H:%M:%S").to_string(),
        ),
        (
            "UTC".into(),
            when.format("%a, %d %b %Y %H:%M:%S GMT").to_string(),
        ),
        (
            "ISO 8601".into(),
            when.to_rfc3339_opts(SecondsFormat::Millis, true),
        ),
        ("Unix 秒".into(), when.timestamp().to_string()),
        ("Unix 毫秒".into(), when.timestamp_millis().to_string()),
        ("相对".into(), relative_time(when)),
    ]
}

fn describe(raw: &str) -> Result<Option<Vec<(String, String)>>, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().all(|ch| ch.is_ascii_digit()) {
        let value: i64 = text.parse().map_err(|_| "时间戳超出范围".to_string())?;
        let (seconds, nanos) = if text.len() <= 10 {
            (value, 0)
        } else {
            (value / 1000, ((value % 1000) * 1_000_000) as u32)
        };
        let when = DateTime::<Utc>::from_timestamp(seconds, nanos)
            .ok_or_else(|| "时间戳超出范围".to_string())?;
        return Ok(Some(fields_for(when)));
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
        return Ok(Some(fields_for(parsed.with_timezone(&Utc))));
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, format) {
            let when = Local
                .from_local_datetime(&naive)
                .single()
                .unwrap_or_else(|| Local.from_utc_datetime(&naive));
            return Ok(Some(fields_for(when.with_timezone(&Utc))));
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0).unwrap_or_default();
        let when = Local
            .from_local_datetime(&naive)
            .single()
            .unwrap_or_else(|| Local.from_utc_datetime(&naive));
        return Ok(Some(fields_for(when.with_timezone(&Utc))));
    }
    Err("无法识别，支持时间戳或 ISO 日期字符串".into())
}

impl TimestampState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        let now = Utc::now().timestamp().to_string();
        Self {
            input: cx.new(|cx| TextInput::single_with(cx, "1700000000 或 2026-11-10T12:00:00", now)),
        }
    }
}

impl DevBoxApp {
    pub fn render_timestamp(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let input = self.pages.timestamp.input.read(cx).text().to_string();
        let result = describe(&input);
        let fields = result.as_ref().ok().and_then(|fields| fields.clone());

        let mut rows = card(&theme).gap(px(0.0)).p(px(0.0));
        match &fields {
            Some(fields) if !fields.is_empty() => {
                for (label, value) in fields {
                    let value_for_copy = value.clone();
                    rows = rows.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .h(px(38.0))
                            .px(px(12.0))
                            .border_b_1()
                            .border_color(theme.line())
                            .child(
                                div()
                                    .w(px(90.0))
                                    .text_size(px(11.0))
                                    .text_color(theme.faint())
                                    .child(label.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_family("Menlo")
                                    .text_size(px(11.5))
                                    .text_color(theme.text())
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(value.clone()),
                            )
                            .child(icon_button(&theme, "⧉").id(crate::ui::loop_id("timestamp_auto2_1", &seq))
.on_click(cx.listener(
                                move |app, _: &ClickEvent, _, cx| {
                                    app.copy(value_for_copy.clone(), cx)
                                },
                            ))),
                    );
                }
            }
            _ => {
                let message = match &result {
                    Err(error) => error.clone(),
                    _ => "输入时间戳或日期字符串".into(),
                };
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py(px(36.0))
                        .text_size(px(11.5))
                        .text_color(if result.is_err() {
                            theme.danger()
                        } else {
                            theme.faint()
                        })
                        .child(message),
                );
            }
        }

        let copy_all = fields
            .as_ref()
            .map(|fields| {
                fields
                    .iter()
                    .map(|(label, value)| format!("{label}: {value}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        tool_page(&theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_header(
                        &theme,
                        "⏱",
                        ORANGE,
                        "时间戳转换",
                        "时间戳 ↔ 日期双向互转",
                    ))
                    .child(
                        panel_actions(&theme)
                            .child(
                                button(&theme, "此刻（秒）", ButtonKind::Subtle).id("timestamp_auto2_2")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        let value = Utc::now().timestamp().to_string();
                                        app.pages.timestamp.input.update(cx, |input, cx| {
                                            input.set_text(value, cx)
                                        });
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "此刻（毫秒）", ButtonKind::Subtle).id("timestamp_auto2_3")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        let value = Utc::now().timestamp_millis().to_string();
                                        app.pages.timestamp.input.update(cx, |input, cx| {
                                            input.set_text(value, cx)
                                        });
                                    }),
                                ),
                            )
                            .child(
                                button(&theme, "复制全部", ButtonKind::Ghost).id("timestamp_auto2_4")
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.copy(copy_all.clone(), cx)
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
                    .child(text_field(&theme, &self.pages.timestamp.input, 36.0))
                    .child(rows),
            )
            .child(
                footer_line(&theme)
                    .child(if input.trim().is_empty() {
                        status_line(&theme, "等待输入", false)
                    } else if result.is_ok() {
                        status_line(&theme, "解析成功", false)
                    } else {
                        status_line(&theme, "无法识别输入", true)
                    })
                    .child(div().child("本地时区渲染")),
            )
    }
}
