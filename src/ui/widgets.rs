//! Reusable visual building blocks shared by the tool pages.

use gpui::{Div, SharedString, div, prelude::*, px};

use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Ghost,
    Danger,
    Subtle,
}

pub fn button(theme: &Theme, label: impl Into<SharedString>, kind: ButtonKind) -> Div {
    let label = label.into();
    let base = div()
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .h(px(30.0))
        .px(px(11.0))
        .rounded(px(8.0))
        .text_size(px(12.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer();
    match kind {
        ButtonKind::Primary => base
            .bg(theme.accent())
            .text_color(gpui::white())
            .hover(|style| style.bg(theme.accent_strong())),
        ButtonKind::Ghost => base
            .bg(theme.card())
            .border_1()
            .border_color(theme.line())
            .text_color(theme.muted())
            .hover(|style| style.border_color(theme.line_strong()).text_color(theme.text())),
        ButtonKind::Danger => base
            .bg(theme.danger_tint())
            .text_color(theme.danger())
            .hover(|style| style.bg(theme.danger()).text_color(gpui::white())),
        ButtonKind::Subtle => base
            .text_color(theme.muted())
            .hover(|style| style.bg(theme.accent_ghost()).text_color(theme.accent())),
    }
    .child(label)
}

pub fn icon_button(theme: &Theme, glyph: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(7.0))
        .text_size(px(12.0))
        .text_color(theme.muted())
        .cursor_pointer()
        .hover(|style| style.bg(theme.accent_ghost()).text_color(theme.accent()))
        .child(glyph.into())
}

pub fn card(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .p(px(16.0))
        .bg(theme.card())
        .border_1()
        .border_color(theme.line())
        .rounded(px(13.0))
}

pub fn inset(theme: &Theme) -> Div {
    div()
        .bg(theme.inset())
        .border_1()
        .border_color(theme.line())
        .rounded(px(9.0))
        .overflow_hidden()
}

pub fn section_header(
    theme: &Theme,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text())
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(theme.faint())
                .child(subtitle.into()),
        )
}

pub fn page_header(
    theme: &Theme,
    glyph: &'static str,
    accent: u32,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(38.0))
                .rounded(px(10.0))
                .bg(gpui::rgba((accent << 8) | 0x1f))
                .text_color(gpui::rgb(accent))
                .text_size(px(15.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(glyph),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .text_size(px(18.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.text())
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.muted())
                        .child(subtitle.into()),
                ),
        )
}

pub fn badge(_theme: &Theme, label: impl Into<SharedString>, color: u32) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .h(px(20.0))
        .px(px(8.0))
        .rounded_full()
        .bg(gpui::rgba((color << 8) | 0x1a))
        .text_color(gpui::rgb(color))
        .text_size(px(10.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(label.into())
}

pub fn dot(color: u32) -> Div {
    div().size(px(6.0)).rounded_full().bg(gpui::rgb(color))
}

pub fn switch(theme: &Theme, on: bool) -> Div {
    let base = div()
        .flex()
        .items_center()
        .w(px(34.0))
        .h(px(19.0))
        .p(px(2.0))
        .rounded_full()
        .cursor_pointer();
    let base = if on {
        base.bg(theme.accent()).justify_end()
    } else {
        base.bg(theme.line_strong()).justify_start()
    };
    base.child(
        div()
            .size(px(15.0))
            .rounded_full()
            .bg(gpui::white())
            .shadow_sm(),
    )
}

pub fn checkbox(theme: &Theme, on: bool) -> Div {
    let base = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(15.0))
        .rounded(px(4.0))
        .border_1()
        .cursor_pointer()
        .text_size(px(10.0))
        .text_color(gpui::white());
    if on {
        base.bg(theme.accent())
            .border_color(theme.accent())
            .child("✓")
    } else {
        base.bg(theme.card()).border_color(theme.line_strong())
    }
}

pub fn checkbox_row(theme: &Theme, label: impl Into<SharedString>, on: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(theme.muted())
        .child(checkbox(theme, on))
        .child(label.into())
}

pub fn field_label(theme: &Theme, label: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.muted())
        .child(label.into())
}

pub fn labeled(
    theme: &Theme,
    label: impl Into<SharedString>,
    input: impl IntoElement,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(field_label(theme, label))
        .child(input)
}

pub fn empty_state(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .py(px(22.0))
        .text_size(px(11.5))
        .text_color(theme.faint())
        .child(text.into())
}

pub fn divider(theme: &Theme) -> Div {
    div().h(px(1.0)).w_full().bg(theme.line())
}

pub fn status_line(theme: &Theme, text: impl Into<SharedString>, error: bool) -> Div {
    let color = if error { theme.danger() } else { theme.muted() };
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(11.0))
        .text_color(color)
        .child(div().size(px(5.0)).rounded_full().bg(color))
        .child(text.into())
}

pub fn kv_line(
    theme: &Theme,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .h(px(30.0))
        .px(px(10.0))
        .border_b_1()
        .border_color(theme.line())
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.faint())
                .child(label.into()),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(theme.text())
                .child(value.into()),
        )
}
