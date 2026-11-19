use std::sync::{Arc, Mutex};

use gpui::{Bounds, Context, Pixels, Render, TestAppContext, div, point, prelude::*, px, size};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn canvas_with_size_full_fills_its_parent(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let recorded: Arc<Mutex<Option<Bounds<Pixels>>>> = Arc::new(Mutex::new(None));
    let sink = recorded.clone();

    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(1000.0), px(1000.0)),
        |_, _| {
            div().size(px(360.0)).child(
                gpui::canvas(
                    move |bounds, _, _| bounds,
                    move |bounds, _, _, _| {
                        *sink.lock().unwrap() = Some(bounds);
                    },
                )
                .size_full(),
            )
        },
    );

    let bounds = recorded.lock().unwrap().unwrap();
    assert_eq!(bounds.size.width, px(360.0), "canvas width: {bounds:?}");
    assert_eq!(bounds.size.height, px(360.0), "canvas height: {bounds:?}");
}

#[gpui::test]
fn canvas_without_size_full_collapses_in_bordered_parent(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let recorded: Arc<Mutex<Option<Bounds<Pixels>>>> = Arc::new(Mutex::new(None));
    let sink = recorded.clone();
    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(1000.0), px(1000.0)),
        |_, _| {
            div()
                .size(px(360.0))
                .border_1()
                .child(gpui::canvas(
                    move |bounds, _, _| bounds,
                    move |bounds, _, _, _| {
                        *sink.lock().unwrap() = Some(bounds);
                    },
                ))
        },
    );
    let bounds = recorded.lock().unwrap().unwrap();
    assert_eq!(bounds.size.height, px(0.0), "expected collapsed canvas: {bounds:?}");
}

#[gpui::test]
fn canvas_in_styled_preview_fills(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let recorded: Arc<Mutex<Option<Bounds<Pixels>>>> = Arc::new(Mutex::new(None));
    let sink = recorded.clone();
    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(1000.0), px(1000.0)),
        |_, _| {
            div()
                .size(px(360.0))
                .bg(gpui::white())
                .border_1()
                .border_color(gpui::black())
                .rounded(px(12.0))
                .overflow_hidden()
                .child(gpui::canvas(
                    move |bounds, _, _| bounds,
                    move |bounds, _, _, _| {
                        *sink.lock().unwrap() = Some(bounds);
                    },
                ).size_full())
        },
    );
    let bounds = recorded.lock().unwrap().unwrap();
    assert_eq!(bounds.size.width, px(358.0), "styled preview canvas bounds {bounds:?}");
    assert_eq!(bounds.size.height, px(358.0), "styled preview canvas bounds {bounds:?}");
}
