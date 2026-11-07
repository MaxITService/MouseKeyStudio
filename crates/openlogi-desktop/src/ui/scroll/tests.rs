use gpui::{
    AppContext as _, Context, InputEvent as _, Modifiers, Render, ScrollDelta, ScrollWheelEvent,
    TestAppContext, VisualTestContext, point, px, size,
};

use super::*;

struct ScrollHarness {
    handle: ScrollHandle,
}

impl Render for ScrollHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SmoothScroll::new("scroll-test")
            .with_handle(&self.handle)
            .scrollbar_mode(ScrollbarMode::Always)
            .size_full()
            .child(div().h(px(2000.)).child("Scrollable content"))
    }
}

fn setup(cx: &mut TestAppContext) -> (ScrollHandle, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init(cx);
    });
    let handle = ScrollHandle::new();
    let fixture_handle = handle.clone();
    let (_, cx) = cx.add_window_view(move |_, _| ScrollHarness {
        handle: fixture_handle,
    });
    cx.simulate_resize(size(px(300.), px(200.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(handle.max_offset().y, px(1800.));
    (handle, cx)
}

fn event(delta: ScrollDelta) -> ScrollWheelEvent {
    ScrollWheelEvent {
        position: point(px(50.), px(50.)),
        delta,
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    }
}

#[gpui::test]
fn rapid_pixel_events_accumulate_and_clamp_before_the_next_frame(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    cx.update(|window, cx| {
        for _ in 0..100 {
            window.dispatch_event(
                event(ScrollDelta::Pixels(point(px(0.), px(-5.)))).to_platform_input(),
                cx,
            );
        }
        assert_eq!(handle.offset().y, px(-500.));
        for _ in 0..100 {
            window.dispatch_event(
                event(ScrollDelta::Pixels(point(px(0.), px(-50.)))).to_platform_input(),
                cx,
            );
        }
        assert_eq!(handle.offset().y, px(-1800.));
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(0.), px(40.)))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset().y, px(-1760.));
    });
}

#[gpui::test]
fn wheel_animation_accumulates_every_event_and_reverses_without_pending_debt(
    cx: &mut TestAppContext,
) {
    let (handle, cx) = setup(cx);
    cx.update(|window, cx| {
        for _ in 0..10 {
            window.dispatch_event(
                event(ScrollDelta::Lines(point(0., -1.))).to_platform_input(),
                cx,
            );
        }
        assert_eq!(handle.offset().y, px(0.), "wheel movement is animated");
        // Switching reduced motion on settles the actual scheduled animation.
        cx.set_reduce_motion(true);
        assert_eq!(
            window.simulate_next_frame(cx),
            1,
            "one frame per region, not per event"
        );
        assert_eq!(handle.offset().y, -window.line_height() * 10.);
    });
}

fn mouse_move(x: f32, y: f32) -> gpui::PlatformInput {
    gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
        position: point(px(x), px(y)),
        pressed_button: None,
        modifiers: Modifiers::default(),
    })
}

#[gpui::test]
fn hand_jitter_keeps_the_wheel_tail_and_a_real_move_ends_it(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    cx.update(|window, cx| {
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -4.))).to_platform_input(),
            cx,
        );
        window.dispatch_event(mouse_move(52., 49.), cx);
        window.dispatch_event(
            ScrollWheelEvent {
                position: point(px(52.), px(49.)),
                ..event(ScrollDelta::Lines(point(0., -4.)))
            }
            .to_platform_input(),
            cx,
        );
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(
            handle.offset().y,
            -window.line_height() * 8.,
            "jitter loses no wheel distance"
        );
        cx.set_reduce_motion(false);
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -4.))).to_platform_input(),
            cx,
        );
        window.dispatch_event(mouse_move(150., 50.), cx);
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(
            handle.offset().y,
            -window.line_height() * 8.,
            "moving to another control drops the tail"
        );
    });
}

#[gpui::test]
fn reduced_motion_and_horizontal_input_keep_direct_platform_behavior(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    cx.update(|window, cx| {
        cx.set_reduce_motion(true);
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -2.))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset().y, -window.line_height() * 2.);
        let before = handle.offset();
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(-100.), px(0.)))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset(), before);
    });
}

#[gpui::test]
fn scrollbar_stays_at_the_viewport_edge_after_scrolling(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    cx.simulate_event(event(ScrollDelta::Pixels(point(px(0.), px(-500.)))));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(handle.offset().y, px(-500.));
    // Click the lower track at the fixed viewport edge, after its content moved.
    cx.simulate_click(point(px(295.), px(180.)), Modifiers::default());
    assert!(
        handle.offset().y < px(-500.),
        "the scrollbar remains reachable"
    );
}

struct NativeScrollHarness {
    handle: ScrollHandle,
}
impl Render for NativeScrollHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("native-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.handle)
            .child(div().h(px(2000.)))
    }
}

#[gpui::test]
fn native_component_wheel_has_no_boundary_debt(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let view_handle = handle.clone();
    let (_, cx) = cx.add_window_view(move |_, _| NativeScrollHarness {
        handle: view_handle,
    });
    cx.simulate_resize(size(px(300.), px(200.)));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        for _ in 0..4 {
            window.dispatch_event(
                event(ScrollDelta::Pixels(point(px(0.), px(60.)))).to_platform_input(),
                cx,
            );
        }
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(0.), px(-60.)))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset().y, px(-60.));
    });
}

#[gpui::test]
fn precise_input_and_navigation_cancel_the_animated_tail(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    cx.update(|window, cx| {
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -10.))).to_platform_input(),
            cx,
        );
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(0.), px(-30.)))).to_platform_input(),
            cx,
        );
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(handle.offset().y, px(-30.));
        cx.set_reduce_motion(false);
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -10.))).to_platform_input(),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("down").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(
            handle.offset().y,
            px(-30.),
            "navigation cancels wheel motion"
        );
    });
}

#[gpui::test]
fn reversal_cancels_unfinished_distance(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    handle.set_offset(point(px(0.), px(-500.)));
    cx.update(|window, cx| {
        for _ in 0..50 {
            window.dispatch_event(
                event(ScrollDelta::Lines(point(0., -1.))).to_platform_input(),
                cx,
            );
        }
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., 1.))).to_platform_input(),
            cx,
        );
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(handle.offset().y, px(-500.) + window.line_height());
    });
}

#[gpui::test]
fn resizing_during_motion_clamps_to_the_new_viewport(cx: &mut TestAppContext) {
    let (handle, cx) = setup(cx);
    cx.update(|window, cx| {
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -100.))).to_platform_input(),
            cx,
        );
    });
    cx.simulate_resize(size(px(300.), px(1900.)));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(handle.max_offset().y, px(100.));
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(handle.offset().y, px(-100.));
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(0.), px(20.)))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset().y, px(-80.));
    });
}

struct HorizontalHarness {
    handle: ScrollHandle,
}
impl Render for HorizontalHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("horizontal")
            .size_full()
            .flex()
            .overflow_x_scroll()
            .track_scroll(&self.handle)
            .children((0..20).map(|i| {
                div()
                    .w(px(100.))
                    .h_full()
                    .flex_shrink_0()
                    .child(format!("Card {i}"))
            }))
    }
}

#[gpui::test]
fn horizontal_wheel_keeps_keyboard_reveal_and_boundary_reversal(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init(cx);
    });
    let handle = ScrollHandle::new();
    let view_handle = handle.clone();
    let (_, cx) = cx.add_window_view(move |_, _| HorizontalHarness {
        handle: view_handle,
    });
    cx.simulate_resize(size(px(300.), px(200.)));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(handle.max_offset().x, px(1700.));
        window.dispatch_event(
            event(ScrollDelta::Lines(point(-3., 0.))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset().x, px(0.));
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(handle.offset().x, -window.line_height() * 3.);
        handle.scroll_to_item(19);
        window.draw(cx).clear(cx);
        assert_eq!(handle.offset().x, px(-1700.));
        for _ in 0..4 {
            window.dispatch_event(
                event(ScrollDelta::Pixels(point(px(-100.), px(0.)))).to_platform_input(),
                cx,
            );
        }
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(20.), px(0.)))).to_platform_input(),
            cx,
        );
        assert_eq!(handle.offset().x, px(-1680.));
    });
}

struct VirtualHarness {
    handle: gpui::UniformListScrollHandle,
}
impl Render for VirtualHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::uniform_list("virtual", 1000, |range, _, _| {
            range
                .map(|i| div().h(px(30.)).child(format!("Application {i}")))
                .collect()
        })
        .track_scroll(&self.handle)
        .size_full()
    }
}

#[gpui::test]
fn virtual_list_uses_common_motion_and_keeps_scroll_to_selection(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init(cx);
    });
    let handle = gpui::UniformListScrollHandle::new();
    let view_handle = handle.clone();
    let (_, cx) = cx.add_window_view(move |_, _| VirtualHarness {
        handle: view_handle,
    });
    cx.simulate_resize(size(px(300.), px(200.)));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let base = handle.0.borrow().base_handle.clone();
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -5.))).to_platform_input(),
            cx,
        );
        assert_eq!(base.offset().y, px(0.));
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(base.offset().y, -window.line_height() * 5.);
        handle.scroll_to_item(999, gpui::ScrollStrategy::Bottom);
        window.draw(cx).clear(cx);
        assert_eq!(base.offset().y, -base.max_offset().y);
        for _ in 0..4 {
            window.dispatch_event(
                event(ScrollDelta::Pixels(point(px(0.), px(-100.)))).to_platform_input(),
                cx,
            );
        }
        window.dispatch_event(
            event(ScrollDelta::Pixels(point(px(0.), px(30.)))).to_platform_input(),
            cx,
        );
        assert_eq!(base.offset().y, -base.max_offset().y + px(30.));
    });
}

struct TextHarness {
    input: gpui::Entity<gpui_component::input::TextareaState>,
}
impl Render for TextHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui_component::input::Textarea::new(&self.input)
            .w_full()
            .h(px(180.))
    }
}

#[gpui::test]
fn multiline_editor_uses_common_motion_with_its_own_line_height(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init(cx);
    });
    let (view, cx) = cx.add_window_view(|window, cx| TextHarness {
        input: cx.new(|cx| {
            let mut input = gpui_component::input::TextareaState::new(window, cx).rows(5);
            input.set_value(
                (0..100)
                    .map(|i| format!("Macro step {i}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                window,
                cx,
            );
            input
        }),
    });
    cx.simulate_resize(size(px(300.), px(200.)));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let input = view.read(cx).input.clone();
        let line_height = input.read(cx).line_height().unwrap();
        window.dispatch_event(
            event(ScrollDelta::Lines(point(0., -3.))).to_platform_input(),
            cx,
        );
        assert_eq!(input.read(cx).scroll_offset().y, px(0.));
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        assert_eq!(input.read(cx).scroll_offset().y, -line_height * 3.);
    });
}

gpui::actions!(scroll_tests, [Activate]);

struct MenuHarness {
    menu: gpui::Entity<gpui_component::menu::PopupMenu>,
}
impl Render for MenuHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.menu.clone())
    }
}

#[gpui::test]
fn toolkit_menu_uses_the_same_wheel_dispatch(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init(cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| MenuHarness {
        menu: gpui_component::menu::PopupMenu::build(window, cx, |mut menu, _, _| {
            for i in 0..30 {
                menu = menu.menu_element(Box::new(Activate), move |_, _| {
                    div()
                        .debug_selector(move || format!("menu-test-{i}"))
                        .child(format!("Action {i}"))
                });
            }
            menu.max_h(px(150.)).scrollable(true)
        }),
    });
    cx.simulate_resize(size(px(300.), px(240.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let before = cx.debug_bounds("menu-test-2").unwrap();
    cx.update(|window, cx| {
        window.dispatch_event(
            ScrollWheelEvent {
                position: before.center(),
                ..event(ScrollDelta::Lines(point(0., -2.)))
            }
            .to_platform_input(),
            cx,
        );
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    let after = cx.debug_bounds("menu-test-2").unwrap();
    assert!(
        after.top() < before.top(),
        "the toolkit's private scroll handle moves"
    );
}

struct EmptyHarness;
impl Render for EmptyHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children(gpui_component::Root::render_dialog_layer(window, cx))
    }
}

#[gpui::test]
fn toolkit_dialog_scrolls_its_body_through_the_same_dispatch(cx: &mut TestAppContext) {
    use gpui_component::WindowExt as _;
    cx.update(|cx| {
        gpui_component::init(cx);
        init(cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| EmptyHarness);
        gpui_component::Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(700.), px(500.)));
    cx.update(|window, cx| {
        cx.set_reduce_motion(true); // Settle the unrelated dialog entrance.
        window.open_dialog(cx, |dialog, _, _| {
            dialog.title("Scroll test").h(px(350.)).child(
                div().h(px(1600.)).flex_shrink_0().child(
                    div()
                        .debug_selector(|| "dialog-scroll-marker".into())
                        .child("Long dialog body"),
                ),
            )
        });
        window.draw(cx).clear(cx);
    });
    let before = cx.debug_bounds("dialog-scroll-marker").unwrap();
    cx.update(|window, cx| {
        cx.set_reduce_motion(false);
        window.dispatch_event(
            ScrollWheelEvent {
                position: before.center(),
                ..event(ScrollDelta::Lines(point(0., -3.)))
            }
            .to_platform_input(),
            cx,
        );
        cx.set_reduce_motion(true);
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    let after = cx.debug_bounds("dialog-scroll-marker").unwrap();
    assert!(
        after.top() < before.top(),
        "before: {before:?}, after: {after:?}"
    );
}
