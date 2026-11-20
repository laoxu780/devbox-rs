use gpui::{ClickEvent, Context, Div, Entity, IntoElement, div, prelude::*, px};
use similar::{ChangeTag, TextDiff};

use crate::ui::app::DevBoxApp;
use crate::ui::text_input::TextInput;
use crate::ui::widgets::ButtonKind;

pub struct DiffState {
    pub left: Entity<TextInput>,
    pub right: Entity<TextInput>,
    pub ignore_case: bool,
    /// Kept for settings compatibility; the diff always uses the four-pane
    /// side-by-side view now.
    pub side_by_side: bool,
}

#[derive(Clone)]
struct DiffRow {
    old: Option<usize>,
    new: Option<usize>,
    text: String,
    tag: ChangeTag,
}

impl DiffState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            left: cx.new(|cx| {
                TextInput::multi_with(
                    cx,
                    "原始文本…",
                    "name: devbox\nversion: 1.0.0\nmode: local\nfeatures:\n  - proxy\n  - ssh",
                )
            }),
            right: cx.new(|cx| {
                TextInput::multi_with(
                    cx,
                    "对比文本…",
                    "name: devbox\nversion: 1.1.0\nmode: local\nfeatures:\n  - proxy\n  - ssh\n  - diff",
                )
            }),
            ignore_case: false,
            side_by_side: true,
        }
    }
}

fn compute_rows(left: &str, right: &str, ignore_case: bool) -> Vec<DiffRow> {
    let (left, right) = if ignore_case {
        (left.to_lowercase(), right.to_lowercase())
    } else {
        (left.to_string(), right.to_string())
    };
    let diff = TextDiff::from_lines(&left, &right);
    diff.iter_all_changes()
        .map(|change| DiffRow {
            old: change.old_index(),
            new: change.new_index(),
            text: change.value().trim_end_matches('\n').to_string(),
            tag: change.tag(),
        })
        .collect()
}

/// Pair consecutive deletions and insertions so they line up vertically.
fn paired_rows(rows: &[DiffRow]) -> Vec<(Option<&DiffRow>, Option<&DiffRow>)> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < rows.len() {
        if rows[index].tag == ChangeTag::Equal {
            out.push((Some(&rows[index]), Some(&rows[index])));
            index += 1;
            continue;
        }
        let mut deleted = Vec::new();
        let mut inserted = Vec::new();
        while index < rows.len() && rows[index].tag != ChangeTag::Equal {
            if rows[index].tag == ChangeTag::Delete {
                deleted.push(&rows[index]);
            } else {
                inserted.push(&rows[index]);
            }
            index += 1;
        }
        for position in 0..deleted.len().max(inserted.len()) {
            out.push((deleted.get(position).copied(), inserted.get(position).copied()));
        }
    }
    out
}

fn area_header(theme: &crate::theme::Theme, label: &str, detail: String) -> Div {
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

fn diff_cell(
    theme: &crate::theme::Theme,
    row: Option<&DiffRow>,
    is_left: bool,
    index: usize,
) -> Div {
    let (number, sign, text, tag) = match row {
        Some(row) => {
            let number = if is_left { row.old } else { row.new };
            let sign = if is_left {
                if row.tag == ChangeTag::Delete { "-" } else { " " }
            } else if row.tag == ChangeTag::Insert {
                "+"
            } else {
                " "
            };
            let tag = if (is_left && row.tag == ChangeTag::Delete)
                || (!is_left && row.tag == ChangeTag::Insert)
            {
                Some(row.tag)
            } else {
                None
            };
            (number, sign, row.text.as_str(), tag)
        }
        None => (None, " ", "", None),
    };

    let (bg, color, marker) = match tag {
        Some(ChangeTag::Delete) => (theme.danger_tint(), theme.danger(), theme.danger()),
        Some(ChangeTag::Insert) => (gpui::rgba(0x2f9e6322), theme.ok(), theme.ok()),
        _ => (
            if index % 2 == 1 { gpui::rgba(0x00000008) } else { gpui::rgba(0x00000000) },
            theme.text(),
            gpui::rgba(0x00000000),
        ),
    };

    div()
        .flex()
        .items_center()
        .h(px(22.0))
        .w_full()
        .bg(bg)
        .child(div().w(px(3.0)).h_full().bg(marker))
        .child(
            div()
                .w(px(40.0))
                .px(px(6.0))
                .text_align(gpui::TextAlign::Right)
                .text_size(px(10.0))
                .text_color(theme.faint())
                .child(number.map(|value| (value + 1).to_string()).unwrap_or_default()),
        )
        .child(
            div()
                .w(px(9.0))
                .text_size(px(11.0))
                .text_color(color)
                .child(sign.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pl(px(4.0))
                .font_family("Menlo")
                .text_size(px(11.0))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .text_color(color)
                .child(text.to_string()),
        )
}

impl DevBoxApp {
    pub fn render_diff(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let left = self.pages.diff.left.read(cx).text().to_string();
        let right = self.pages.diff.right.read(cx).text().to_string();
        let ignore_case = self.pages.diff.ignore_case;
        let rows = compute_rows(&left, &right, ignore_case);
        let pairs = paired_rows(&rows);
        let inserts = rows.iter().filter(|row| row.tag == ChangeTag::Insert).count();
        let deletes = rows.iter().filter(|row| row.tag == ChangeTag::Delete).count();

        let left_input = div()
            .debug_selector(|| "diff_input_left".into())
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
            .child(area_header(&theme, "原始文本", format!("{} 行", left.lines().count())))
            .child(div().flex_1().min_h_0().child(self.pages.diff.left.clone()));

        let right_input = div()
            .debug_selector(|| "diff_input_right".into())
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
            .child(area_header(&theme, "对比文本", format!("{} 行", right.lines().count())))
            .child(div().flex_1().min_h_0().child(self.pages.diff.right.clone()));

        let mut left_pane = div()
            .debug_selector(|| "diff_result_left".into())
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
            .id("diff_left_pane")
            .overflow_scroll();
        let mut right_pane = div()
            .debug_selector(|| "diff_result_right".into())
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
            .id("diff_right_pane")
            .overflow_scroll();
        for (index, (old, new)) in pairs.iter().enumerate() {
            left_pane = left_pane.child(diff_cell(&theme, *old, true, index));
            right_pane = right_pane.child(diff_cell(&theme, *new, false, index));
        }

        let diff_header = area_header(
            &theme,
            "对比结果",
            format!("+{inserts} · -{deletes} · {} 行", rows.len() - inserts - deletes),
        );

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .gap(px(10.0))
            .debug_selector(|| "diff_root".into())
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
                            .child("文本对比"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme.muted())
                                    .child(format!("+{inserts} 行 · -{deletes} 行")),
                            )
                            .child(
                                crate::ui::widgets::button(
                                    &theme,
                                    if ignore_case { "忽略大小写：开" } else { "忽略大小写：关" },
                                    ButtonKind::Subtle,
                                )
                                .id("diff_ignore_case")
                                .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                    app.pages.diff.ignore_case = !app.pages.diff.ignore_case;
                                    cx.notify();
                                })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .flex_1()
                    .min_h_0()
                    .child(left_input)
                    .child(right_input),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .flex_1()
                    .min_h_0()
                    .child(diff_header)
                    .child(
                        div()
                            .flex()
                            .gap(px(10.0))
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .child(left_pane),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .child(right_pane),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_deletions_with_insertions() {
        let rows = vec![
            DiffRow { old: Some(0), new: Some(0), text: "a".into(), tag: ChangeTag::Equal },
            DiffRow { old: Some(1), new: None, text: "old".into(), tag: ChangeTag::Delete },
            DiffRow { old: None, new: Some(1), text: "new".into(), tag: ChangeTag::Insert },
            DiffRow { old: Some(2), new: Some(2), text: "c".into(), tag: ChangeTag::Equal },
        ];
        let pairs = paired_rows(&rows);
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[1].0.unwrap().text, "old");
        assert_eq!(pairs[1].1.unwrap().text, "new");
    }
}
