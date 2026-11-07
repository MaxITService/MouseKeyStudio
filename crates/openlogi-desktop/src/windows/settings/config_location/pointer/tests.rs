use super::*;

#[test]
fn path_wrapping_preserves_the_visible_path_and_does_not_change_its_length() {
    let path = r"D:\Portable Programs\Mouse & Key Studio\mousekey-studio-portable.toml";
    let wrapped = wrapping_path(path);
    assert!(wrapped.contains('\u{200b}'));
    assert_eq!(wrapped.replace('\u{200b}', ""), path);
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use crate::services::i18n::LOCALE_LOCK;
    use gpui::{
        KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, px, size,
    };

    #[gpui::test]
    fn copy_is_exact_by_pointer_and_keyboard_and_denied_move_is_inert(cx: &mut TestAppContext) {
        let _locale = LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        cx.update(gpui_component::init);
        let mut view = None;
        let handle = cx.open_window(size(px(520.), px(1100.)), |window, cx| {
            let pointer = cx.new(|_| {
                PointerView::example(ConfigPointerLocation::UserProfile, Some("Access denied"))
            });
            view = Some(pointer.clone());
            gpui_component::Root::new(pointer, window, cx)
        });
        let view = view.unwrap();
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let expected = view.read_with(&visual, |view, _| {
            view.destinations.as_ref().unwrap()[1].path.clone()
        });
        let copy = visual.debug_bounds("copy-pointer-portable").unwrap();
        visual.simulate_click(copy.center(), Modifiers::default());
        assert_eq!(
            visual.update(|_, cx| cx.read_from_clipboard().unwrap().text().unwrap()),
            expected
        );
        let denied = visual.debug_bounds("config-pointer-portable").unwrap();
        visual.simulate_click(denied.center(), Modifiers::default());
        view.read_with(&visual, |view, _| {
            assert!(!view.busy, "disabled move must not start filesystem work");
            assert!(view.destinations.as_ref().unwrap()[0].selected);
        });

        // Tab navigation skips both disabled move buttons. Space must run
        // the same copy command after explicitly focusing the second tab stop.
        visual.update(|window, cx| {
            window.blur(cx);
            window.focus_next(cx);
            window.focus_next(cx);
        });
        visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
        let keystroke = Keystroke::parse("space").unwrap();
        visual.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        visual.simulate_event(KeyUpEvent { keystroke });
        assert_eq!(
            visual.update(|_, cx| cx.read_from_clipboard().unwrap().text().unwrap()),
            expected
        );
        let user_copy = visual.debug_bounds("copy-pointer-user").unwrap();
        assert_eq!(
            user_copy.left(),
            copy.left(),
            "related actions share their leading edge"
        );
        assert!(
            copy.right() <= px(520.),
            "long paths must not push actions outside the panel"
        );
    }
}
