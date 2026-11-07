//! Real toolkit scroll surfaces, captured after native wheel dispatch.

use crate::ui::{
    snapshots::Scene,
    theme::{self, Typography as _},
};
use gpui::{
    AnyView, App, AppContext as _, Context, Entity, InputEvent as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollDelta, ScrollHandle, ScrollWheelEvent,
    StatefulInteractiveElement as _, Styled as _, UniformListScrollHandle, Window, div, point, px,
    size, uniform_list,
};
use gpui_component::{
    Root, WindowExt as _, h_flex,
    input::{Textarea, TextareaState},
    menu::PopupMenu,
    scroll::ScrollableElement as _,
    v_flex,
};
use std::time::Duration;

gpui::actions!(scroll_snapshots, [Choose]);

pub(crate) fn scenes() -> Vec<Scene> {
    vec![
        Scene {
            name: "scroll/components",
            size: size(px(1080.), px(620.)),
            build: components,
        },
        Scene {
            name: "scroll/dialog",
            size: size(px(800.), px(600.)),
            build: dialog,
        },
    ]
}

fn wheel_after_layout(window: &mut Window, cx: &mut App, positions: Vec<(f32, f32)>) {
    window
        .spawn(cx, async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            for (x, y) in positions {
                let _ = cx.update(|window, cx| {
                    for _ in 0..8 {
                        window.dispatch_event(
                            ScrollWheelEvent {
                                position: point(px(x), px(y)),
                                delta: ScrollDelta::Lines(point(0., -1.)),
                                ..Default::default()
                            }
                            .to_platform_input(),
                            cx,
                        );
                    }
                });
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
        })
        .detach();
}

struct Components {
    menu: Entity<PopupMenu>,
    input: Entity<TextareaState>,
    list: UniformListScrollHandle,
    horizontal: ScrollHandle,
}

fn components(window: &mut Window, cx: &mut App) -> AnyView {
    let menu = PopupMenu::build(window, cx, |mut menu, _, _| {
        for i in 0..50 {
            menu = menu.menu(format!("Action {}", i + 1), Box::new(Choose));
        }
        menu.max_h(px(300.)).scrollable(true)
    });
    let input = cx.new(|cx| {
        let mut input = TextareaState::new(window, cx).rows(10);
        input.set_value(
            (0..100)
                .map(|i| format!("+LeftCtrl, {}ms, C, -LeftCtrl", 50 + i))
                .collect::<Vec<_>>()
                .join("\n"),
            window,
            cx,
        );
        input
    });
    let view = cx.new(|_| Components {
        menu,
        input,
        list: UniformListScrollHandle::new(),
        horizontal: ScrollHandle::new(),
    });
    wheel_after_layout(window, cx, vec![(150., 220.), (510., 220.), (900., 220.)]);
    // Horizontal input gets its own animation after the vertical examples.
    window
        .spawn(cx, async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(340))
                .await;
            let _ = cx.update(|window, cx| {
                window.dispatch_event(
                    ScrollWheelEvent {
                        position: point(px(500.), px(530.)),
                        delta: ScrollDelta::Lines(point(-8., 0.)),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
            });
        })
        .detach();
    view.into()
}

impl Render for Components {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = theme::palette(cx);
        let panel = |title: &'static str, content: gpui::AnyElement| {
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_3()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(pal.border)
                .child(div().text_subheading().child(title))
                .child(content)
        };
        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .text_color(pal.text_primary)
            .child(
                div()
                    .text_heading()
                    .child("Shared scrolling — native components"),
            )
            .child(
                h_flex()
                    .h(px(350.))
                    .gap_4()
                    .child(panel("Menu", self.menu.clone().into_any_element()))
                    .child(panel(
                        "Application catalog",
                        div()
                            .relative()
                            .w_full()
                            .h(px(300.))
                            .child(
                                uniform_list("scroll-catalog", 1000, |range, _, _| {
                                    range
                                        .map(|i| {
                                            div()
                                                .h(px(32.))
                                                .px_2()
                                                .child(format!("Application {}", i + 1))
                                        })
                                        .collect()
                                })
                                .track_scroll(&self.list)
                                .w_full()
                                .h_full(),
                            )
                            .vertical_scrollbar(&self.list)
                            .into_any_element(),
                    ))
                    .child(panel(
                        "Macro text",
                        Textarea::new(&self.input)
                            .h(px(300.))
                            .w_full()
                            .into_any_element(),
                    )),
            )
            .child(div().text_subheading().child("Horizontal cards"))
            .child(
                div()
                    .relative()
                    .h(px(115.))
                    .w_full()
                    .child(
                        h_flex()
                            .id("scroll-horizontal-cards")
                            .size_full()
                            .gap_3()
                            .overflow_x_scroll()
                            .track_scroll(&self.horizontal)
                            .children((0..14).map(|i| {
                                div()
                                    .flex_shrink_0()
                                    .w(px(180.))
                                    .h(px(90.))
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(pal.border)
                                    .child(format!("Device {}", i + 1))
                            })),
                    )
                    .horizontal_scrollbar(&self.horizontal),
            )
    }
}

struct DialogHost;
impl Render for DialogHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children(Root::render_dialog_layer(window, cx))
    }
}

fn dialog(window: &mut Window, cx: &mut App) -> AnyView {
    window.on_next_frame(|window, cx| {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("Applications using this profile")
                .h(px(460.))
                .child(v_flex().flex_shrink_0().gap_3().children((0..40).map(|i| {
                    v_flex()
                        .gap_1()
                        .child(format!("Application {}", i + 1))
                        .child(div().text_sm().child(format!(
                            r"C:\Program Files\Example Vendor\Application {}\app.exe",
                            i + 1
                        )))
                })))
        });
    });
    wheel_after_layout(window, cx, vec![(400., 280.)]);
    cx.new(|_| DialogHost).into()
}
