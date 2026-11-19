use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};

use crate::ports::{ListenEntry, PortStatus, check_ports, list_listeners, parse_port_input};
use crate::ui::app::{DevBoxApp, ORANGE};
use crate::ui::pages::{footer_line, io_head, panel_actions, segmented, text_field, tool_page, chip};
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ButtonKind, badge, button, card, dot, empty_state, page_header, status_line};

pub struct PortState {
    pub query: Entity<TextInput>,
    pub filter: Entity<TextInput>,
    pub checks: Vec<PortStatus>,
    pub listeners: Vec<ListenEntry>,
    pub protocol: String,
    pub notice: String,
    pub notice_error: bool,
    pub last_refresh: String,
}

impl PortState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        let mut state = Self {
            query: cx.new(|cx| TextInput::single(cx, "如 8080, 3000-3010")),
            filter: cx.new(|cx| TextInput::single(cx, "过滤端口 / 进程名")),
            checks: Vec::new(),
            listeners: Vec::new(),
            protocol: "all".into(),
            notice: String::new(),
            notice_error: false,
            last_refresh: String::new(),
        };
        state.refresh();
        state
    }

    pub fn refresh(&mut self) {
        match list_listeners() {
            Ok(listeners) => {
                self.listeners = listeners;
                self.notice.clear();
                self.notice_error = false;
                self.last_refresh = chrono::Local::now()
                    .format("%H:%M:%S")
                    .to_string();
            }
            Err(error) => {
                self.notice = error;
                self.notice_error = true;
            }
        }
    }

    pub fn check(&mut self, cx: &mut Context<DevBoxApp>) {
        let input = self.query.read(cx).text().to_string();
        let ports = parse_port_input(&input);
        if ports.is_empty() {
            self.notice = "请输入有效端口，例如 8080 或 3000-3010".into();
            self.notice_error = true;
            self.checks.clear();
            return;
        }
        self.checks = check_ports(&ports, &self.listeners);
        self.notice.clear();
        self.notice_error = false;
    }
}

impl DevBoxApp {
    pub fn render_ports(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = std::cell::Cell::new(0);
        let theme = self.theme;
        let state = &self.pages.ports;
        let protocol = state.protocol.clone();
        let notice = state.notice.clone();
        let notice_error = state.notice_error;

        let mut protocol_switch = segmented(&theme);
        for (value, label) in [
            ("all", "全部"),
            ("tcp", "TCP"),
            ("udp", "UDP"),
        ] {
            let selector: &'static str = match value {
                "tcp" => "ports_protocol_tcp",
                "udp" => "ports_protocol_udp",
                _ => "ports_protocol_all",
            };
            protocol_switch = protocol_switch.child(
                chip(&theme, label, protocol == value)
                    .id(selector)
                    .debug_selector(move || selector.into())
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.pages.ports.protocol = value.to_string();
                        cx.notify();
                    })),
            );
        }

        // -- port availability results -------------------------------------
        let mut results = card(&theme).gap(px(0.0)).p(px(0.0));
        if state.checks.is_empty() {
            results = results.child(empty_state(&theme, "输入端口后点击「检测端口」"));
        }
        for status in &state.checks {
            let port = status.port.to_string();
            let port_selector = port.clone();
            let selector: &'static str = if status.available {
                "ports_result_free"
            } else {
                "ports_result_busy"
            };
            results = results.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(32.0))
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(theme.line())
                    .id(crate::ui::loop_id(selector, &seq))
                    .debug_selector(move || port_selector.clone().into())
                    .child(dot(if status.available { theme.ok } else { theme.danger }))
                    .child(
                        div()
                            .w(px(64.0))
                            .font_family("Menlo")
                            .text_size(px(12.0))
                            .text_color(theme.text())
                            .child(port.clone()),
                    )
                    .child(
                        div()
                            .font_family("Menlo")
                            .text_size(px(11.0))
                            .text_color(if status.available {
                                theme.muted()
                            } else {
                                gpui::rgb(theme.danger)
                            })
                            .child(status.detail.clone()),
                    ),
            );
        }

        // -- listener list --------------------------------------------------
        let filter = state.filter.read(cx).text().trim().to_lowercase();
        let shown: Vec<&ListenEntry> = state
            .listeners
            .iter()
            .filter(|entry| match protocol.as_str() {
                "tcp" => entry.protocol == "TCP",
                "udp" => entry.protocol == "UDP",
                _ => true,
            })
            .filter(|entry| {
                filter.is_empty()
                    || entry.port.to_string().contains(&filter)
                    || entry.address.to_lowercase().contains(&filter)
                    || entry.process.to_lowercase().contains(&filter)
            })
            .collect();
        let mut rows = card(&theme).gap(px(0.0)).p(px(0.0));
        if shown.is_empty() {
            rows = rows.child(empty_state(&theme, "没有匹配的监听端口"));
        }
        for entry in shown {
            let port_value = entry.port.to_string();
            let fill = port_value.clone();
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(30.0))
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(theme.line())
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.accent_ghost()))
                    .id(crate::ui::loop_id("ports_listener_row", &seq))
                    .debug_selector({
                        let selector = format!("ports_listener_{}", entry.port);
                        move || selector.clone().into()
                    })
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.pages
                            .ports
                            .query
                            .update(cx, |input, cx| input.set_text(fill.clone(), cx));
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(38.0))
                            .text_size(px(10.0))
                            .text_color(if entry.protocol == "TCP" {
                                gpui::rgb(theme.accent)
                            } else {
                                gpui::rgb(ORANGE)
                            })
                            .child(entry.protocol.clone()),
                    )
                    .child(
                        div()
                            .w(px(140.0))
                            .font_family("Menlo")
                            .text_size(px(11.0))
                            .text_color(theme.muted())
                            .child(entry.address.clone()),
                    )
                    .child(
                        div()
                            .w(px(56.0))
                            .font_family("Menlo")
                            .text_size(px(11.5))
                            .text_color(theme.text())
                            .child(port_value),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.0))
                            .text_color(theme.text())
                            .child(entry.process.clone()),
                    )
                    .child(
                        div()
                            .w(px(56.0))
                            .font_family("Menlo")
                            .text_size(px(10.0))
                            .text_color(theme.faint())
                            .child(format!("PID {}", entry.pid)),
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
                        "P",
                        ORANGE,
                        "端口检测 / 占用查询",
                        "查看本机监听端口，检测端口是否可用",
                    ))
                    .child(
                        panel_actions(&theme)
                            .child(protocol_switch)
                            .child(
                                button(&theme, "刷新列表", ButtonKind::Subtle)
                                    .id("ports_refresh")
                                    .debug_selector(|| "ports_refresh".into())
                                    .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.ports.refresh();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(7.0))
                    .child(io_head(
                        &theme,
                        "端口可用性检测",
                        "支持逗号、空格和范围，如 8080, 3000-3010",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(text_field(&theme, &self.pages.ports.query, 34.0))
                            .child(
                                button(&theme, "检测端口", ButtonKind::Primary)
                                    .id("ports_check")
                                    .debug_selector(|| "ports_check".into())
                                    .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                        app.pages.ports.check(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(results),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(7.0))
                    .child(io_head(
                        &theme,
                        "本机监听端口",
                        format!(
                            "{} 个 · {}",
                            state.listeners.len(),
                            if state.last_refresh.is_empty() {
                                "启动后自动扫描".into()
                            } else {
                                format!("刷新于 {}", state.last_refresh)
                            }
                        ),
                    ))
                    .child(text_field(&theme, &self.pages.ports.filter, 32.0))
                    .child(rows),
            )
            .child(
                footer_line(&theme)
                    .child(if notice.is_empty() {
                        status_line(&theme, "点击列表中的端口可填入上方检测框", false)
                    } else {
                        status_line(&theme, notice, notice_error)
                    })
                    .child(badge(&theme, format!("{} 条结果", state.checks.len()), theme.accent)),
            )
    }
}
