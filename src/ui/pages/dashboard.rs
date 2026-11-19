use gpui::{ClickEvent, Context, IntoElement, div, prelude::*, px};

use crate::theme::Theme;
use crate::ui::app::{DevBoxApp, ToolId, VIOLET, tool_meta};
use crate::ui::pages::{chip, tool_page};
use crate::ui::widgets::{
    ButtonKind, badge, button, card, dot, page_header, section_header, status_line, switch,
};

#[derive(Default)]
pub struct DashboardState {}

fn stat_card(theme: &Theme, value: String, label: &str, hint: String, accent: u32) -> impl IntoElement {
    card(theme)
        .flex_1()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(24.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(gpui::rgb(accent))
                        .child(value),
                )
                .child(dot(accent)),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text())
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(10.5))
                .text_color(theme.faint())
                .child(hint),
        )
}

impl DevBoxApp {
    pub fn render_dashboard(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let routes = self.services.proxy.list_routes();
        let forwards = self.services.ssh.list_forwards();
        let profiles = self.services.ssh.list_profiles();
        let keys = self.services.ssh.list_keys();

        let running_routes = routes.iter().filter(|route| route.status == "running").count();
        let running_forwards = forwards
            .iter()
            .filter(|forward| forward.status == "running")
            .count();

        let mut page = tool_page(&theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_header(
                        &theme,
                        "D",
                        VIOLET,
                        "仪表盘",
                        "总览运行状态，快速切换环境与转发",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                button(&theme, "⌘K 命令面板", ButtonKind::Ghost).id("dashboard_auto2_1")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| app.open_palette(cx)),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(stat_card(
                        &theme,
                        format!("{running_routes}/{}", routes.len()),
                        "接口转发",
                        "本地监听 · 运行时切换".into(),
                        VIOLET,
                    ))
                    .child(stat_card(
                        &theme,
                        format!("{running_forwards}/{}", forwards.len()),
                        "SSH 转发",
                        "隧道与代理模式".into(),
                        0x12939b,
                    ))
                    .child(stat_card(
                        &theme,
                        profiles.len().to_string(),
                        "SSH 凭证",
                        "服务器连接配置".into(),
                        0xc27a2c,
                    ))
                    .child(stat_card(
                        &theme,
                        keys.len().to_string(),
                        "SSH 密钥",
                        "~/.ssh 与 DevBox 密钥".into(),
                        0xc2548f,
                    )),
            );

        // --- proxy routes ---------------------------------------------------
        let mut route_section = card(&theme);
        route_section = route_section.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(section_header(&theme, "接口环境转发", "监听端口保持不变，随时切换后端"))
                .child(
                    button(&theme, "管理规则", ButtonKind::Subtle).id("dashboard_auto2_2")
.on_click(cx.listener(
                        |app, _: &ClickEvent, _, cx| app.select_tool(ToolId::ApiProxy, cx),
                    )),
                ),
        );
        if routes.is_empty() {
            route_section = route_section.child(
                div()
                    .py(px(10.0))
                    .text_size(px(11.5))
                    .text_color(theme.faint())
                    .child("还没有接口转发规则，前往「接口环境转发」创建。"),
            );
        }
        for route in &routes {
            let route_id = route.id.clone();
            let mut row = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .bg(theme.card2())
                .border_1()
                .border_color(theme.line())
                .rounded(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text())
                                .child(route.name.clone()),
                        )
                        .child(badge(
                            &theme,
                            if route.status == "running" { "运行中" } else { "已停止" },
                            if route.status == "running" { theme.ok } else { theme.faint },
                        ))
                        .child(div().flex_1())
                        .child(
                            div()
                                .font_family("Menlo")
                                .text_size(px(11.0))
                                .text_color(theme.muted())
                                .child(route.local_address.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme.faint())
                                .child(format!("{} 次请求", route.requests)),
                        ),
                );
            let mut targets = div().flex().flex_wrap().gap(px(6.0));
            for target in &route.targets {
                let route_id = route_id.clone();
                let target_id = target.id.clone();
                let active = target.id == route.active_target_id;
                targets = targets.child(chip(&theme, target.name.clone(), active).id(crate::ui::loop_id("dashboard_auto2_3", &seq))
.on_click(
                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                        match app.services.proxy.switch_target(&route_id, &target_id) {
                            Ok(status) => {
                                let message = format!("已切换到 {}", status.active_target.name);
                                app.notify(message, false, cx)
                            }
                            Err(error) => app.notify(error, true, cx),
                        }
                    }),
                ));
            }
            row = row.child(targets);
            route_section = route_section.child(row);
        }
        page = page.child(route_section);

        // --- ssh forwards ---------------------------------------------------
        let mut forward_section = card(&theme);
        forward_section = forward_section.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(section_header(&theme, "SSH 端口转发", "一键启停本机隧道"))
                .child(
                    button(&theme, "管理转发", ButtonKind::Subtle).id("dashboard_auto2_4")
.on_click(cx.listener(
                        |app, _: &ClickEvent, _, cx| app.select_tool(ToolId::SshForward, cx),
                    )),
                ),
        );
        if forwards.is_empty() {
            forward_section = forward_section.child(
                div()
                    .py(px(10.0))
                    .text_size(px(11.5))
                    .text_color(theme.faint())
                    .child("还没有 SSH 转发规则。"),
            );
        }
        for forward in &forwards {
            let id = forward.id.clone();
            let enabled = forward.status != "stopped";
            let mode = match forward.mode.as_str() {
                "socks" => "SOCKS5",
                "http" => "HTTP",
                _ => "本地",
            };
            let running = forward.status == "running";
            forward_section = forward_section.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .p(px(11.0))
                    .bg(theme.card2())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(10.0))
                    .child(dot(if running {
                        theme.ok
                    } else if forward.status == "error" {
                        theme.danger
                    } else {
                        theme.faint
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.text())
                                    .child(forward.name.clone()),
                            )
                            .child(
                                div()
                                    .font_family("Menlo")
                                    .text_size(px(10.5))
                                    .text_color(theme.muted())
                                    .child(format!(
                                        "{} → {}",
                                        forward.local_address, forward.remote_address
                                    )),
                            ),
                    )
                    .child(badge(&theme, mode, theme.accent))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme.faint())
                            .child(format!("{} 连接", forward.connections)),
                    )
                    .child(switch(&theme, enabled).id(crate::ui::loop_id("dashboard_auto2_5", &seq))
.on_click(cx.listener(
                        move |app, _: &ClickEvent, _, cx| {
                            match app.services.ssh.toggle_forward(&id, !enabled) {
                                Ok(message) => app.notify(message, false, cx),
                                Err(error) => app.notify(error, true, cx),
                            }
                        },
                    ))),
            );
        }
        page = page.child(forward_section);

        // --- quick tools ----------------------------------------------------
        let mut grid = div().flex().flex_wrap().gap(px(8.0));
        for tool in crate::ui::app::TOOLS.iter().filter(|tool| {
            matches!(
                tool.id,
                ToolId::Json
                    | ToolId::Base64
                    | ToolId::Timestamp
                    | ToolId::Uuid
                    | ToolId::Hash
                    | ToolId::Regex
            )
        }) {
            let id = tool.id;
            grid = grid.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .w(px(210.0))
                    .p(px(10.0))
                    .bg(theme.card())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(10.0))
                    .cursor_pointer()
                    .hover(|style| style.border_color(theme.accent()))
                    .id(crate::ui::loop_id("dashboard_auto_1", &seq))

                    .id(crate::ui::loop_id("dashboard_auto2_6", &seq))
.on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.select_tool(id, cx)
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(28.0))
                            .rounded(px(8.0))
                            .bg(gpui::rgba((tool.accent << 8) | 0x1f))
                            .text_color(gpui::rgb(tool.accent))
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(tool.glyph),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.text())
                                    .child(tool.name),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(theme.faint())
                                    .text_ellipsis()
                                    .child(tool_meta(id).description),
                            ),
                    ),
            );
        }
        page = page.child(card(&theme).gap(px(10.0)).child(section_header(
            &theme,
            "常用工具",
            "本地计算，不上传任何内容",
        )).child(grid));

        page.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(status_line(
                    &theme,
                    format!(
                        "{} 条转发规则 · {} 个凭证 · {} 个密钥",
                        forwards.len(),
                        profiles.len(),
                        keys.len()
                    ),
                    false,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(dot(theme.ok))
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.faint())
                                .child("服务运行中"),
                        ),
                ),
        )
    }
}
