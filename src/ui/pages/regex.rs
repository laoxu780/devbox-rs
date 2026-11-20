use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};
use regex::RegexBuilder;

use crate::ui::app::{BLUE, DevBoxApp};
use crate::ui::pages::{footer_line, io_head, panel_actions, text_area, tool_page};
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{card, checkbox_row, page_header, status_line};

pub struct RegexState {
    pub pattern: Entity<TextInput>,
    pub flags: String,
    pub text: Entity<TextInput>,
}

pub struct RegexMatch {
    pub text: String,
    pub index: usize,
    pub groups: Vec<String>,
    pub named: Vec<(String, String)>,
}

pub struct RegexResult {
    pub error: Option<String>,
    pub lines: Vec<Vec<(String, bool)>>,
    pub matches: Vec<RegexMatch>,
}

fn run_regex(pattern: &str, flags: &str, text: &str) -> RegexResult {
    if pattern.is_empty() || text.is_empty() {
        return RegexResult {
            error: None,
            lines: Vec::new(),
            matches: Vec::new(),
        };
    }
    let built = RegexBuilder::new(pattern)
        .case_insensitive(flags.contains('i'))
        .multi_line(flags.contains('m'))
        .dot_matches_new_line(flags.contains('s'))
        .unicode(true)
        .build();
    let regex = match built {
        Ok(regex) => regex,
        Err(error) => {
            return RegexResult {
                error: Some(error.to_string()),
                lines: Vec::new(),
                matches: Vec::new(),
            };
        }
    };
    let names: Vec<Option<String>> = regex
        .capture_names()
        .map(|name| name.map(str::to_string))
        .collect();
    let mut matches = Vec::new();
    let mut segments: Vec<(String, bool)> = Vec::new();
    let mut last = 0usize;
    for captures in regex.captures_iter(text).take(1000) {
        let Some(found) = captures.get(0) else {
            continue;
        };
        if found.start() > last {
            segments.push((text[last..found.start()].to_string(), false));
        }
        segments.push((found.as_str().to_string(), true));
        let groups = (1..captures.len())
            .map(|index| {
                captures
                    .get(index)
                    .map(|group| group.as_str().to_string())
                    .unwrap_or_default()
            })
            .collect();
        let named = names
            .iter()
            .skip(1)
            .flatten()
            .filter_map(|name| {
                captures
                    .name(name)
                    .map(|value| (name.clone(), value.as_str().to_string()))
            })
            .collect();
        matches.push(RegexMatch {
            text: found.as_str().to_string(),
            index: found.start(),
            groups,
            named,
        });
        last = found.end();
    }
    if last < text.len() {
        segments.push((text[last..].to_string(), false));
    }

    let mut lines: Vec<Vec<(String, bool)>> = vec![Vec::new()];
    for (segment, is_match) in segments {
        let mut parts = segment.split('\n').peekable();
        while let Some(part) = parts.next() {
            if !part.is_empty() {
                lines
                    .last_mut()
                    .unwrap()
                    .push((part.to_string(), is_match));
            }
            if parts.peek().is_some() {
                lines.push(Vec::new());
            }
        }
    }

    RegexResult {
        error: None,
        lines,
        matches,
    }
}

impl RegexState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            pattern: cx.new(|cx| {
                TextInput::single_with(cx, "输入正则表达式…", r"(\w+)@(\w+)\.(\w+)")
            }),
            flags: "gi".into(),
            text: cx.new(|cx| {
                TextInput::multi_with(
                    cx,
                    "输入测试文本…",
                    "联系 alice@example.com 或 bob@test.org\n无效地址: carol#example.net",
                )
            }),
        }
    }
}

const REGEX_FLAGS: [(&str, &str); 5] = [
    ("g", "全局"),
    ("i", "忽略大小写"),
    ("m", "多行"),
    ("s", "点号匹配换行"),
    ("u", "Unicode"),
];

impl DevBoxApp {
    pub fn render_regex(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let pattern = self.pages.regex.pattern.read(cx).text().to_string();
        let flags = self.pages.regex.flags.clone();
        let text = self.pages.regex.text.read(cx).text().to_string();
        let result = run_regex(&pattern, &flags, &text);
        let match_count = result.matches.len();

        let mut flag_row = panel_actions(&theme);
        for (flag, label) in REGEX_FLAGS {
            let flag_string = flag.to_string();
            flag_row = flag_row.child(
                checkbox_row(&theme, flag, flags.contains(flag))
                    .id(crate::ui::loop_id("regex_auto_1", &seq))

                    .id(crate::ui::loop_id("regex_auto2_1", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        if app.pages.regex.flags.contains(&flag_string) {
                            app.pages.regex.flags = app.pages.regex.flags.replace(&flag_string, "");
                        } else {
                            app.pages.regex.flags.push_str(&flag_string);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint())
                            .child(label),
                    ),
            );
        }

        let mut preview = div()
            .flex()
            .flex_col()
            .gap(px(1.0))
            .p(px(10.0))
            .h(px(240.0))
            .w_full()
            .bg(theme.card())
            .border_1()
            .border_color(theme.line())
            .rounded(px(9.0))
            .id("regex_auto_2")

            .id("regex_auto2_2")
.overflow_scroll()
            .font_family("Menlo")
            .text_size(px(11.5));
        if result.error.is_some() {
            preview = preview.child(
                div()
                    .text_color(theme.danger())
                    .child(result.error.clone().unwrap_or_default()),
            );
        } else if pattern.is_empty() || text.is_empty() {
            preview = preview.child(
                div()
                    .text_color(theme.faint())
                    .child("输入表达式与测试文本"),
            );
        } else if match_count == 0 {
            preview = preview.child(
                div()
                    .text_color(theme.faint())
                    .child("没有匹配"),
            );
        } else {
            for line in &result.lines {
                let mut row = div().flex().items_center().min_h(px(17.0));
                if line.is_empty() {
                    row = row.child(div().child(" "));
                }
                for (segment, is_match) in line {
                    let mut span = div().whitespace_nowrap().child(segment.clone());
                    if *is_match {
                        span = span
                            .bg(gpui::rgba((BLUE << 8) | 0x40))
                            .text_color(theme.text())
                            .rounded(px(3.0))
                            .px(px(1.0));
                    } else {
                        span = span.text_color(theme.muted());
                    }
                    row = row.child(span);
                }
                preview = preview.child(row);
            }
        }

        let mut match_list = div().flex().flex_col().gap(px(2.0));
        for (index, found) in result.matches.iter().take(100).enumerate() {
            let mut groups = String::new();
            for (group_index, group) in found.groups.iter().enumerate() {
                if group.is_empty() {
                    groups.push_str(&format!("${}=∅  ", group_index + 1));
                } else {
                    groups.push_str(&format!("${}={}  ", group_index + 1, group));
                }
            }
            for (name, value) in &found.named {
                groups.push_str(&format!("{name}={value}  "));
            }
            match_list = match_list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .h(px(26.0))
                    .px(px(9.0))
                    .bg(theme.card2())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(6.0))
                    .text_size(px(10.5))
                    .child(
                        div()
                            .w(px(28.0))
                            .text_color(theme.faint())
                            .child(format!("#{}", index + 1)),
                    )
                    .child(
                        div()
                            .font_family("Menlo")
                            .text_color(theme.text())
                            .child(if found.text.is_empty() {
                                "(空匹配)".to_string()
                            } else {
                                found.text.clone()
                            }),
                    )
                    .child(
                        div()
                            .text_color(theme.faint())
                            .child(format!("@{}", found.index)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_color(theme.muted())
                            .child(groups),
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
                        ".*",
                        BLUE,
                        "正则测试",
                        "实时匹配、分组捕获与高亮",
                    ))
                    .child(flag_row),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .h(px(36.0))
                    .px(px(10.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(8.0))
                    .font_family("Menlo")
                    .text_size(px(12.0))
                    .text_color(theme.faint())
                    .child("/")
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .child(self.pages.regex.pattern.clone()),
                    )
                    .child(format!("/{flags}")),
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
                                "测试文本",
                                format!("{} 字符", text.chars().count()),
                            ))
                            .child(text_area(&theme, &self.pages.regex.text, 240.0)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(io_head(
                                &theme,
                                "匹配结果",
                                if result.error.is_some() {
                                    "表达式无效".into()
                                } else {
                                    format!("{match_count} 处匹配")
                                },
                            ))
                            .child(preview),
                    ),
            )
            .children(if match_count > 0 {
                Some(
                    card(&theme)
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.faint())
                                .child(format!("匹配明细（前 100 条，共 {match_count} 条）")),
                        )
                        .child(match_list),
                )
            } else {
                None
            })
            .child(
                footer_line(&theme)
                    .child(if pattern.is_empty() {
                        status_line(&theme, "等待表达式", false)
                    } else if result.error.is_some() {
                        status_line(
                            &theme,
                            result.error.clone().unwrap_or_default(),
                            true,
                        )
                    } else if text.is_empty() {
                        status_line(&theme, "等待测试文本", false)
                    } else {
                        status_line(&theme, format!("{match_count} 处匹配"), false)
                    })
                    .child(div().child("本地执行")),
            )
    }
}
