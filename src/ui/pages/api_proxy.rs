use std::time::Duration;

use gpui::{ClickEvent, Context, Entity, IntoElement, MouseDownEvent, div, prelude::*, px};

use crate::proxy::{ProxyRoute, ProxyStatus, ProxyTarget};
use crate::ui::app::{BLUE, ConfirmAction, ConfirmState, DevBoxApp, ToolId};
use crate::ui::pages::{footer_line, panel_actions, text_field, tool_page};
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{
    ButtonKind, badge, button, card, checkbox_row, dot, empty_state, icon_button, labeled,
    page_header, section_header, status_line, switch,
};

pub struct TargetForm {
    pub id: String,
    pub name: Entity<TextInput>,
    pub url: Entity<TextInput>,
    pub insecure: bool,
}

impl TargetForm {
    fn new(cx: &mut Context<DevBoxApp>, name: &str, url: &str) -> Self {
        Self {
            id: String::new(),
            name: cx.new(|cx| TextInput::single_with(cx, "环境名称", name.to_string())),
            url: cx.new(|cx| TextInput::single_with(cx, "https://api.example.com", url.to_string())),
            insecure: false,
        }
    }
}

pub struct ProxyForm {
    pub id: String,
    pub name: Entity<TextInput>,
    pub host: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub strip: Entity<TextInput>,
    pub cors: bool,
    pub targets: Vec<TargetForm>,
}

fn watch(input: &Entity<TextInput>, cx: &mut Context<DevBoxApp>) {
    cx.observe(input, |_, _, cx| cx.notify()).detach();
}

impl ProxyForm {
    fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            id: String::new(),
            name: cx.new(|cx| TextInput::single(cx, "本地接口代理")),
            host: cx.new(|cx| TextInput::single_with(cx, "127.0.0.1", "127.0.0.1")),
            port: cx.new(|cx| TextInput::single_with(cx, "8787", "8787")),
            strip: cx.new(|cx| TextInput::single(cx, "/api，转发时去掉")),
            cors: false,
            targets: vec![
                TargetForm::new(cx, "开发环境", ""),
                TargetForm::new(cx, "生产环境", ""),
            ],
        }
    }

    fn reset(&mut self, cx: &mut Context<DevBoxApp>) {
        self.id.clear();
        self.name.update(cx, |input, cx| input.clear(cx));
        self.host
            .update(cx, |input, cx| input.set_text("127.0.0.1", cx));
        self.port
            .update(cx, |input, cx| input.set_text("8787", cx));
        self.strip.update(cx, |input, cx| input.clear(cx));
        self.cors = false;
        if self.targets.len() < 2 {
            while self.targets.len() < 2 {
                let target = TargetForm::new(cx, "", "");
                watch(&target.name, cx);
                watch(&target.url, cx);
                self.targets.push(target);
            }
        }
        self.targets.truncate(2);
        for (index, target) in self.targets.iter_mut().enumerate() {
            target.id.clear();
            target.insecure = false;
            let name = if index == 0 { "开发环境" } else { "生产环境" };
            target
                .name
                .update(cx, |input, cx| input.set_text(name, cx));
            target.url.update(cx, |input, cx| input.clear(cx));
        }
    }

    fn load(&mut self, route: &ProxyStatus, cx: &mut Context<DevBoxApp>) {
        self.id = route.id.clone();
        self.name
            .update(cx, |input, cx| input.set_text(route.name.clone(), cx));
        self.host.update(cx, |input, cx| {
            input.set_text(
                if route.listen_host.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    route.listen_host.clone()
                },
                cx,
            )
        });
        self.port
            .update(cx, |input, cx| input.set_text(route.listen_port.to_string(), cx));
        self.strip.update(cx, |input, cx| {
            input.set_text(route.strip_prefix.clone(), cx)
        });
        self.cors = route.cors;

        while self.targets.len() < route.targets.len() {
            let target = TargetForm::new(cx, "", "");
            watch(&target.name, cx);
            watch(&target.url, cx);
            self.targets.push(target);
        }
        self.targets.truncate(route.targets.len());
        for (index, source) in route.targets.iter().enumerate() {
            let target = &mut self.targets[index];
            target.id = source.id.clone();
            target.insecure = source.insecure;
            let name = source.name.clone();
            let url = source.url.clone();
            target
                .name
                .update(cx, |input, cx| input.set_text(name, cx));
            target.url.update(cx, |input, cx| input.set_text(url, cx));
        }
    }

    fn add_target(&mut self, cx: &mut Context<DevBoxApp>) {
        let target = TargetForm::new(cx, "", "");
        watch(&target.name, cx);
        watch(&target.url, cx);
        self.targets.push(target);
    }

    fn target_count(&self) -> usize {
        self.targets.len()
    }
}

pub struct ApiProxyState {
    pub form: ProxyForm,
    pub show_form: bool,
    pub notice: String,
    pub notice_error: bool,
    pub switching: String,
}

impl ApiProxyState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        // Refresh request counters and externally-triggered switches while the
        // page is visible, mirroring the web UI's two second poll.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(2))
                    .await;
                let alive = this
                    .update(cx, |app, cx| {
                        if app.tool == ToolId::ApiProxy {
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
        Self {
            form: ProxyForm::new(cx),
            show_form: false,
            notice: String::new(),
            notice_error: false,
            switching: String::new(),
        }
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.is_empty()
        || matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
        || host.starts_with("127.")
}

fn route_state(route: &ProxyStatus) -> (u32, &'static str) {
    if route.disabled {
        (0, "已关闭")
    } else if route.status == "running" {
        (1, "运行中")
    } else if route.status == "error" {
        (3, "启动失败")
    } else {
        (2, "已停止")
    }
}

impl DevBoxApp {
    pub fn render_api_proxy(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let routes = self.services.proxy.list_routes();
        let notice = self.pages.api_proxy.notice.clone();
        let notice_error = self.pages.api_proxy.notice_error;
        let switching = self.pages.api_proxy.switching.clone();

        let mut page = tool_page(&theme).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(page_header(
                    &theme,
                    "⇄",
                    BLUE,
                    "接口环境转发",
                    "本地端口保持不变，一键切换后端环境",
                ))
                .child(
                    panel_actions(&theme).child(
                        button(
                            &theme,
                            if !self.pages.api_proxy.form.id.is_empty() {
                                "编辑转发"
                            } else {
                                "新增转发"
                            },
                            ButtonKind::Primary,
                        )
                        .debug_selector(|| "api_proxy_add_button".into())
                        .id("api_proxy_auto_1")

                        .id("api_proxy_auto2_1")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                            if app.pages.api_proxy.show_form {
                                app.pages.api_proxy.show_form = false;
                            } else {
                                app.pages.api_proxy.form.reset(cx);
                                app.pages.api_proxy.show_form = true;
                                app.pages.api_proxy.notice.clear();
                                app.pages.api_proxy.notice_error = false;
                            }
                            cx.notify();
                        })),
                    ),
                ),
        );

        page = page.child(
            card(&theme)
                .gap(px(4.0))
                .child(section_header(
                    &theme,
                    "前端代理指向本地端口，后端环境一键切换",
                    "Vite 示例：server.proxy['/api'] = { target: 'http://127.0.0.1:8787', changeOrigin: true }",
                ))
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(theme.faint())
                        .child("切换只改变转发目标，本地端口与前端代理配置保持不变。"),
                ),
        );

        page = page.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(2.0))
                .text_size(px(11.0))
                .text_color(theme.muted())
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("转发规则"),
                )
                .child(format!("{} 条 · 切换环境无需重启前端", routes.len())),
        );

        let mut list = div().flex().flex_col().gap(px(8.0));
        if routes.is_empty() {
            list = list.child(card(&theme).child(empty_state(
                &theme,
                "暂无接口转发，点击右上角「新增转发」创建",
            )));
        }
        for route in &routes {
            let running = route.status == "running";
            let (state_kind, state_text) = route_state(route);
            let dot_color = match state_kind {
                1 => theme.ok,
                3 => theme.danger,
                2 => 0xc27a2c,
                _ => theme.faint,
            };
            let disabled = route.disabled;
            let id_toggle = route.id.clone();
            let id_retry = route.id.clone();
            let id_delete = route.id.clone();
            let name_delete = route.name.clone();
            let route_for_edit = route.clone();
            let local_address = route.local_address.clone();

            let mut title = div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text())
                        .child(route.name.clone()),
                );
            if !route.strip_prefix.is_empty() {
                title = title.child(badge(
                    &theme,
                    format!("去掉 {}", route.strip_prefix),
                    theme.accent,
                ));
            }
            if route.cors {
                title = title.child(badge(&theme, "CORS", theme.ok));
            }

            let mut envs = div().flex().flex_wrap().gap(px(6.0));
            for target in &route.targets {
                let active = target.id == route.active_target_id;
                let route_id = route.id.clone();
                let target_id = target.id.clone();
                let target_name = target.name.clone();
                let url_short = target
                    .url
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .to_string();
                let pending = switching == format!("{}:{}", route.id, target.id);
                let clickable = running && !active;
                let mut env = div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(9.0))
                    .py(px(6.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if active {
                        theme.accent()
                    } else {
                        theme.line()
                    })
                    .bg(if active {
                        theme.accent_tint()
                    } else {
                        theme.card2()
                    });
                if clickable {
                    env = env.cursor_pointer().hover(|style| {
                        style
                            .border_color(theme.accent())
                            .bg(theme.accent_ghost())
                    });
                }
                let env = env
                    .child(dot(if active { theme.accent } else { theme.faint }))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(if active {
                                gpui::FontWeight::SEMIBOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .text_color(if active {
                                theme.accent()
                            } else {
                                theme.text()
                            })
                            .child(if pending {
                                format!("{target_name} · 切换中…")
                            } else {
                                target_name
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme.faint())
                            .child(url_short),
                    );
                let env = if clickable {
                    env.id(gpui::ElementId::Name(format!("proxy-env-{route_id}-{target_id}").into()))
                        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                            app.pages.api_proxy.switching =
                                format!("{route_id}:{target_id}");
                            cx.notify();
                            match app
                                .services
                                .proxy
                                .switch_target(&route_id, &target_id)
                            {
                                Ok(status) => {
                                    app.pages.api_proxy.notice = format!(
                                        "已切换到 {} · {}",
                                        status.active_target.name, status.active_target.url
                                    );
                                    app.pages.api_proxy.notice_error = false;
                                    app.notify(
                                        format!("已切换到 {}", status.active_target.name),
                                        false,
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    app.pages.api_proxy.notice = error.clone();
                                    app.pages.api_proxy.notice_error = true;
                                    app.notify(error, true, cx);
                                }
                            }
                            app.pages.api_proxy.switching.clear();
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    env.into_any_element()
                };
                envs = envs.child(env);
            }

            let mut meta = div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .text_size(px(10.0))
                .text_color(theme.faint())
                .child(state_text)
                .child(format!("{} 次请求", route.requests));
            if let Some(recent) = route.recent.first() {
                meta = meta.child(format!(
                    "最近：{} {} · {} · {}ms",
                    recent.method, recent.path, recent.status, recent.duration_ms
                ));
            }
            if running {
                meta = meta.child("POST /__devbox__/switch?target=环境名");
            }

            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(9.0))
                .p(px(12.0))
                .bg(if route.disabled {
                    theme.card2()
                } else {
                    theme.card()
                })
                .border_1()
                .border_color(theme.line())
                .rounded(px(11.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .child(dot(dot_color))
                        .child(div().flex_1().min_w_0().child(title))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .font_family("Menlo")
                                        .text_size(px(11.0))
                                        .text_color(theme.muted())
                                        .child(if local_address.is_empty() {
                                            "未监听".to_string()
                                        } else {
                                            local_address.clone()
                                        }),
                                )
                                .child(icon_button(&theme, "⧉").id(crate::ui::loop_id("api_proxy_auto2_9", &seq))
.on_click(cx.listener(
                                    move |app, _: &ClickEvent, _, cx| {
                                        app.copy(local_address.clone(), cx)
                                    },
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    switch(&theme, !disabled).id(crate::ui::loop_id("api_proxy_auto2_10", &seq))
.on_click(cx.listener(
                                        move |app, _: &ClickEvent, _, cx| {
                                            match app
                                                .services
                                                .proxy
                                                .toggle_route(&id_toggle, disabled)
                                            {
                                                Ok(message) => app.notify(message, false, cx),
                                                Err(error) => app.notify(error, true, cx),
                                            }
                                        },
                                    )),
                                )
                                .child(
                                    button(&theme, "编辑", ButtonKind::Subtle).id(crate::ui::loop_id("api_proxy_auto2_11", &seq))
.on_click(
                                        cx.listener(move |app, _: &ClickEvent, _, cx| {
                                            app.pages
                                                .api_proxy
                                                .form
                                                .load(&route_for_edit, cx);
                                            app.pages.api_proxy.show_form = true;
                                            app.pages.api_proxy.notice.clear();
                                            app.pages.api_proxy.notice_error = false;
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .child(
                                    button(&theme, "删除", ButtonKind::Danger).id(crate::ui::loop_id("api_proxy_auto2_12", &seq))
.on_click(
                                        cx.listener(move |app, _: &ClickEvent, _, cx| {
                                            app.confirm = Some(ConfirmState {
                                                title: "删除接口转发".into(),
                                                message: format!(
                                                    "确定删除「{}」？本地监听端口会立即关闭。",
                                                    name_delete
                                                ),
                                                action: ConfirmAction::DeleteProxyRoute(
                                                    id_delete.clone(),
                                                ),
                                            });
                                            cx.notify();
                                        }),
                                    ),
                                ),
                        ),
                )
                .child(envs)
                .child(meta);
            if !route.error.is_empty() {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .p(px(8.0))
                        .bg(theme.danger_tint())
                        .rounded(px(7.0))
                        .text_size(px(10.5))
                        .text_color(theme.danger())
                        .child(route.error.clone())
                        .children(if route.disabled {
                            None
                        } else {
                            Some(
                                button(&theme, "重试", ButtonKind::Ghost).id(crate::ui::loop_id("api_proxy_auto2_13", &seq))
.on_click(
                                    cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        match app.services.proxy.toggle_route(&id_retry, true) {
                                            Ok(message) => app.notify(message, false, cx),
                                            Err(error) => app.notify(error, true, cx),
                                        }
                                    }),
                                ),
                            )
                        }),
                );
            }
            list = list.child(body);
        }

        page = page.child(list);

        page.child(
            footer_line(&theme)
                .child(if notice.is_empty() {
                    status_line(
                        &theme,
                        "规则保存在本机，应用启动时自动恢复 · 切换环境不会断开前端连接",
                        false,
                    )
                } else {
                    status_line(&theme, notice, notice_error)
                })
                .child(div().child(format!("{} 条规则", routes.len()))),
        )
    }
    /// Modal for adding / editing an interface forwarding rule.
    pub fn render_api_proxy_form_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let seq = std::cell::Cell::new(0);
        let editing = !self.pages.api_proxy.form.id.is_empty();
        let target_count = self.pages.api_proxy.form.target_count();
        let listen_host = self
            .pages
            .api_proxy
            .form
            .host
            .read(cx)
            .text()
            .to_string();
        let cors = self.pages.api_proxy.form.cors;

            let form = &self.pages.api_proxy.form;
            let mut targets = div().flex().flex_col().gap(px(6.0));
            for index in 0..target_count {
                let target = &self.pages.api_proxy.form.targets[index];
                targets = targets.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .child(
                            div()
                                .w(px(150.0))
                                .child(text_field(&theme, &target.name, 32.0)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(text_field(&theme, &target.url, 32.0)),
                        )
                        .child(
                            checkbox_row(&theme, "忽略证书", target.insecure).id(crate::ui::loop_id("api_proxy_auto2_2", &seq))
.on_click(
                                cx.listener(move |app, _: &ClickEvent, _, cx| {
                                    if let Some(target) =
                                        app.pages.api_proxy.form.targets.get_mut(index)
                                    {
                                        target.insecure = !target.insecure;
                                    }
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            button(&theme, "移除", ButtonKind::Subtle).id(crate::ui::loop_id("api_proxy_auto2_3", &seq))
.on_click(
                                cx.listener(move |app, _: &ClickEvent, _, cx| {
                                    if app.pages.api_proxy.form.targets.len() > 1 {
                                        app.pages.api_proxy.form.targets.remove(index);
                                    }
                                    cx.notify();
                                }),
                            ),
                        ),
                );
            }
            targets = targets.child(
                button(&theme, "添加环境", ButtonKind::Ghost).id("api_proxy_auto2_4")
.on_click(cx.listener(
                    |app, _: &ClickEvent, _, cx| {
                        app.pages.api_proxy.form.add_target(cx);
                        cx.notify();
                    },
                )),
            );

            let mut body = card(&theme)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(section_header(
                            &theme,
                            if editing { "编辑接口转发" } else { "新增接口转发" },
                            "每个环境是一个后端根地址，可带路径前缀",
                        ))
                        .child(
                            button(&theme, "✕", ButtonKind::Subtle)
                                .debug_selector(|| "api_proxy_dialog_close".into())
                                .id("api_proxy_dialog_close")
                                .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                    app.pages.api_proxy.show_form = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled(
                                    &theme,
                                    "规则名称",
                                    text_field(&theme, &form.name, 34.0),
                                )),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled(
                                    &theme,
                                    "路径前缀（可选）",
                                    text_field(&theme, &form.strip, 34.0),
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled(
                                    &theme,
                                    "监听地址",
                                    text_field(&theme, &form.host, 34.0),
                                )),
                        )
                        .child(div().pb(px(8.0)).text_color(theme.faint()).child(":"))
                        .child(
                            div()
                                .w(px(120.0))
                                .child(labeled(
                                    &theme,
                                    "端口（0 随机）",
                                    text_field(&theme, &form.port, 34.0),
                                )),
                        ),
                );
            if !is_loopback_host(listen_host.trim()) {
                body = body.child(status_line(
                    &theme,
                    "监听非回环地址时，局域网内其他设备也能访问该端口以及 /__devbox__/ 管理接口。",
                    true,
                ));
            }
            body = body
                .child(
                    checkbox_row(&theme, "允许跨域（自动响应 CORS 预检）", cors).id("api_proxy_auto2_5")
.on_click(
                        cx.listener(|app, _: &ClickEvent, _, cx| {
                            app.pages.api_proxy.form.cors = !app.pages.api_proxy.form.cors;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt(px(2.0))
                        .text_size(px(11.0))
                        .text_color(theme.muted())
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("后端环境"),
                        )
                        .child("创建后点击环境标签即可切换"),
                )
                .child(targets)
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            button(&theme, "取消", ButtonKind::Ghost).id("api_proxy_auto2_6")
.on_click(
                                cx.listener(|app, _: &ClickEvent, _, cx| {
                                    app.pages.api_proxy.show_form = false;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            button(
                                &theme,
                                if editing { "保存修改" } else { "创建并开启" },
                                ButtonKind::Primary,
                            )
                            .id("api_proxy_auto_2")

                            .id("api_proxy_auto2_7")
.on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                let form = &app.pages.api_proxy.form;
                                let listen_port = form
                                    .port
                                    .read(cx)
                                    .text()
                                    .trim()
                                    .parse::<i64>()
                                    .unwrap_or(0);
                                let targets = form
                                    .targets
                                    .iter()
                                    .map(|target| ProxyTarget {
                                        id: target.id.clone(),
                                        name: target.name.read(cx).text().to_string(),
                                        url: target.url.read(cx).text().to_string(),
                                        insecure: target.insecure,
                                    })
                                    .collect::<Vec<_>>();
                                let active_target_id = app
                                    .services
                                    .proxy
                                    .list_routes()
                                    .into_iter()
                                    .find(|route| route.id == form.id)
                                    .map(|route| route.active_target_id)
                                    .unwrap_or_default();
                                let route = ProxyRoute {
                                    id: form.id.clone(),
                                    name: form.name.read(cx).text().to_string(),
                                    listen_host: form.host.read(cx).text().to_string(),
                                    listen_port,
                                    strip_prefix: form.strip.read(cx).text().to_string(),
                                    cors: form.cors,
                                    disabled: false,
                                    position: 0,
                                    active_target_id,
                                    targets,
                                };
                                let edited = !route.id.is_empty();
                                let result = if edited {
                                    app.services.proxy.update_route(route)
                                } else {
                                    app.services.proxy.start_route(route)
                                };
                                match result {
                                    Ok(status) => {
                                        app.pages.api_proxy.show_form = false;
                                        app.pages.api_proxy.notice = format!(
                                            "{}：{} → {}",
                                            if edited { "规则已更新" } else { "转发已创建" },
                                            status.local_address,
                                            status.active_target.name
                                        );
                                        app.pages.api_proxy.notice_error = false;
                                        app.pages.api_proxy.form.reset(cx);
                                        app.notify(
                                            if edited {
                                                "接口转发已更新"
                                            } else {
                                                "接口转发已创建"
                                            },
                                            false,
                                            cx,
                                        );
                                    }
                                    Err(error) => {
                                        app.pages.api_proxy.notice = error.clone();
                                        app.pages.api_proxy.notice_error = true;
                                        app.notify(error, true, cx);
                                    }
                                }
                                cx.notify();
                            })),
                        ),
                );

        crate::ui::pages::modal_shell(
            760.0,
            "api_proxy_dialog",
            cx.listener(|app, _: &MouseDownEvent, _, cx| {
                app.pages.api_proxy.show_form = false;
                cx.notify();
            }),
            body,
        )
    }

}
