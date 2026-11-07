//! Shared action-catalog rows and menu surfaces used by binding editors.

use crate::ui::components::DetailedTooltip as _;
use std::rc::Rc;

use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px, rgb,
    svg,
};
use gpui_component::{
    Icon, IconName, Selectable as _, button::Button, h_flex, input::InputState, v_flex,
};
use openlogi_core::binding::{Action, ApplicationTarget, Category, GestureDirection};

pub(crate) use self::shortcut_recorder::{Recording, RecordsShortcut, ShortcutRecorder};
use crate::ui::action::localized_action_label;
use crate::ui::components::{MenuRow, control_input};
use crate::ui::section::section_label;
use crate::ui::theme::{ACCENT, Palette, Typography as _};

mod shell_help;
mod shortcut_recorder;
pub(crate) use shell_help::CommandDraft;
#[cfg(all(debug_assertions, feature = "ui-snapshots"))]
pub(crate) use shell_help::snapshot_scenes as command_snapshot_scenes;

/// Height cap shared by compact inspector and editor lists.
pub(crate) const EDITOR_LIST_MAX_H: f32 = 360.;

/// Commit callback invoked when an action row is clicked.
pub(crate) type PickFn = Rc<dyn Fn(Action, &mut Window, &mut App)>;

/// The action catalog grouped by [`Category`], preserving catalog order within
/// each group and first-seen order across groups.
pub(crate) fn grouped_catalog() -> Vec<(Category, Vec<Action>)> {
    let mut sections: Vec<(Category, Vec<Action>)> = Vec::new();
    for action in Action::catalog() {
        let category = action.category();
        if let Some(section) = sections
            .iter_mut()
            .find(|(existing, _)| *existing == category)
        {
            section.1.push(action);
        } else {
            sections.push((category, vec![action]));
        }
    }
    sections
}

/// Icon for a gesture-mode button's five-direction summary.
pub(crate) const GESTURE_BUTTON_ICON: &str = "action-icons/move.svg";

/// Icon for one gesture direction: an arrow away from the centre, or the
/// centre itself for the click. Four come from gpui-component's bundled set
/// and the dot is vendored, but all five are lucide at the same stroke weight
/// as the action icons they sit above.
pub(crate) fn gesture_direction_icon(direction: GestureDirection) -> Icon {
    match direction {
        GestureDirection::Up => Icon::new(IconName::ArrowUp),
        GestureDirection::Down => Icon::new(IconName::ArrowDown),
        GestureDirection::Left => Icon::new(IconName::ArrowLeft),
        GestureDirection::Right => Icon::new(IconName::ArrowRight),
        GestureDirection::Click => Icon::empty().path("action-icons/circle-dot.svg"),
    }
}

/// Asset path of the vendored glyph for an action. Exhaustive so a new action
/// must deliberately choose an icon.
pub(crate) fn action_icon_path(action: &Action) -> &'static str {
    match action {
        Action::None => "action-icons/ban.svg",
        Action::LeftClick | Action::RightClick => "action-icons/mouse-pointer-click.svg",
        Action::MiddleClick => "action-icons/mouse.svg",
        Action::MouseBack => "action-icons/circle-arrow-left.svg",
        Action::MouseForward => "action-icons/circle-arrow-right.svg",
        Action::Copy => "action-icons/copy.svg",
        Action::Paste => "action-icons/clipboard-paste.svg",
        Action::Cut => "action-icons/scissors.svg",
        Action::Undo => "action-icons/undo-2.svg",
        Action::Redo => "action-icons/redo-2.svg",
        Action::SelectAll | Action::Workflow(_) => "action-icons/list-checks.svg",
        Action::Find => "action-icons/search.svg",
        Action::Save => "action-icons/save.svg",
        Action::BrowserBack => "action-icons/arrow-left.svg",
        Action::BrowserForward => "action-icons/arrow-right.svg",
        Action::NewTab => "action-icons/square-plus.svg",
        Action::CloseTab => "action-icons/square-x.svg",
        Action::ReopenTab => "action-icons/rotate-ccw.svg",
        Action::NextTab | Action::NextKeypadPage => "action-icons/chevron-right.svg",
        Action::PrevTab | Action::PreviousKeypadPage => "action-icons/chevron-left.svg",
        Action::ReloadPage => "action-icons/rotate-cw.svg",
        Action::MissionControl | Action::ShowActionsRing => "action-icons/layout-grid.svg",
        Action::AppExpose | Action::GShift => "action-icons/layers.svg",
        Action::PreviousDesktop => "action-icons/square-arrow-left.svg",
        Action::NextDesktop => "action-icons/square-arrow-right.svg",
        Action::ShowDesktop => "action-icons/monitor.svg",
        Action::LaunchpadShow | Action::OpenApplication(_) => "action-icons/grid-3x3.svg",
        Action::LockScreen => "action-icons/lock.svg",
        Action::Screenshot | Action::CaptureRegion => "action-icons/camera.svg",
        Action::Sleep => "action-icons/moon.svg",
        Action::PlayPause => "action-icons/play.svg",
        Action::NextTrack => "action-icons/skip-forward.svg",
        Action::PrevTrack => "action-icons/skip-back.svg",
        Action::VolumeUp => "action-icons/volume-2.svg",
        Action::VolumeDown => "action-icons/volume-1.svg",
        Action::MuteVolume => "action-icons/volume-x.svg",
        Action::CycleDpiPresets
        | Action::SetDpiPreset(_)
        | Action::TogglePrecisionMode
        | Action::DpiShift => "action-icons/gauge.svg",
        Action::ToggleSmartShift => "action-icons/refresh-cw.svg",
        Action::ScrollUp | Action::NextDpiPreset => "action-icons/chevrons-up.svg",
        Action::ScrollDown | Action::PreviousDpiPreset => "action-icons/chevrons-down.svg",
        Action::HorizontalScrollLeft => "action-icons/chevrons-left.svg",
        Action::HorizontalScrollRight => "action-icons/chevrons-right.svg",
        Action::CustomShortcut(_)
        | Action::HoldShortcut(_)
        | Action::TapKeyHoldingModifiers(_)
        | Action::TypeText(_)
        | Action::Macro(_) => "action-icons/keyboard.svg",
        Action::RunAppleScript(_)
        | Action::RunShellCommand(_)
        | Action::RunPowerShellCommand(_) => "action-icons/terminal.svg",
    }
}

/// Build every category-grouped action row.
pub(crate) fn action_rows(
    id_prefix: &'static str,
    current: Option<&Action>,
    on_pick: &PickFn,
    pal: Palette,
) -> Vec<gpui::Div> {
    action_rows_matching(id_prefix, current, "", on_pick, pal)
}

/// Build action rows filtered by localized action or category name.
pub(crate) fn action_rows_matching(
    id_prefix: &'static str,
    current: Option<&Action>,
    query: &str,
    on_pick: &PickFn,
    pal: Palette,
) -> Vec<gpui::Div> {
    action_rows_with(
        id_prefix,
        current,
        query,
        ActionOffer::default(),
        on_pick,
        pal,
    )
}

/// What a picker offers beyond the catalog, and what of it it leaves out.
#[derive(Clone, Copy, Default)]
pub(crate) struct ActionOffer<'a> {
    pub extra: &'a [Action],
    pub hidden: &'a [Action],
}

pub(crate) fn action_rows_with(
    id_prefix: &'static str,
    current: Option<&Action>,
    query: &str,
    offer: ActionOffer<'_>,
    on_pick: &PickFn,
    pal: Palette,
) -> Vec<gpui::Div> {
    let query = query.trim().to_lowercase();
    let mut catalog_index = 0usize;
    let mut sections = Vec::new();
    if let Some(action) = searched_shortcut(&query).filter(|a| !offer.hidden.contains(a)) {
        sections.push(
            v_flex()
                .child(editor_section(tr!("actions.keyboard_key_or_shortcut"), pal))
                .child(action_row(
                    id_prefix,
                    usize::MAX,
                    action,
                    current,
                    on_pick,
                    pal,
                )),
        );
    }
    for (category, mut actions) in grouped_catalog() {
        actions.extend(
            offer
                .extra
                .iter()
                .filter(|a| a.category() == category)
                .cloned(),
        );
        actions.retain(|action| !offer.hidden.contains(action));
        let category_label = rust_i18n::t!(category.translation_key());
        let category_matches = category_label.to_lowercase().contains(&query);
        // Number the full catalog before filtering so typing in the search box
        // never changes an action row's element identity.
        let actions: Vec<(usize, Action)> = actions
            .into_iter()
            .map(|action| {
                let key = catalog_index;
                catalog_index += 1;
                (key, action)
            })
            .filter(|(_, action)| {
                query.is_empty()
                    || category_matches
                    || localized_action_label(action)
                        .to_lowercase()
                        .contains(&query)
                    || action.label().to_lowercase().contains(&query)
            })
            .collect();
        if actions.is_empty() {
            continue;
        }
        sections.push(
            v_flex()
                .child(editor_section(category_label.into_owned(), pal))
                .children(actions.into_iter().map(|(action_key, action)| {
                    action_row(id_prefix, action_key, action, current, on_pick, pal)
                })),
        );
    }
    sections
}

/// A typed key query is an immediately assignable action, not just a catalog filter.
fn searched_shortcut(query: &str) -> Option<Action> {
    query.trim().parse().ok().map(Action::CustomShortcut)
}

fn action_row(
    id_prefix: &'static str,
    action_key: usize,
    action: Action,
    current: Option<&Action>,
    on_pick: &PickFn,
    pal: Palette,
) -> impl IntoElement {
    let selected = current == Some(&action);
    let label = localized_action_label(&action);
    let accessible_label = label.clone();
    let icon_path = action_icon_path(&action);
    let tip = tr!("actions.action_assign_tooltip", action => label);
    let on_pick = on_pick.clone();
    MenuRow::new((id_prefix, action_key))
        .debug_selector(|| {
            if action_key == usize::MAX {
                format!("{id_prefix}-keyboard-query")
            } else {
                format!("{id_prefix}-{action:?}")
            }
        })
        .selected(selected)
        .role(Role::MenuItem)
        .aria_label(accessible_label)
        .tooltip(move |window, cx| crate::ui::components::detailed_tooltip(tip.clone(), window, cx))
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    svg()
                        .path(icon_path)
                        .size_4()
                        .flex_none()
                        .text_color(pal.text_muted),
                )
                .child(div().child(label)),
        )
        .when(selected, |row| {
            row.child(
                div()
                    .debug_selector(|| format!("{id_prefix}-{action:?}-check"))
                    .flex_none()
                    .size_3()
                    .child(Icon::new(IconName::Check).size_3().text_color(rgb(ACCENT))),
            )
        })
        .on_click(move |_event, window, cx| (on_pick)(action.clone(), window, cx))
}

/// Shared card surface for compact binding panels and menus.
pub(crate) fn compact_panel(pal: Palette) -> gpui::Div {
    v_flex()
        .bg(pal.panel)
        .border_1()
        .border_color(pal.border)
        .rounded(pal.card_radius)
        .shadow_md()
        .p_1p5()
}

/// A group heading inset to line up with the rows under it.
pub(crate) fn editor_section(label: impl Into<gpui::SharedString>, pal: Palette) -> gpui::Div {
    section_label(label, pal).w_full().px_2().pt_2().pb_0p5()
}

/// Compact editor title.
pub(crate) fn title(text: impl Into<gpui::SharedString>, pal: Palette) -> impl IntoElement {
    div()
        .px_2()
        .pb_1()
        .text_subheading()
        .text_color(pal.text_muted)
        .child(text.into())
}

/// Hairline separating compact editor regions.
pub(crate) fn divider(pal: Palette) -> impl IntoElement {
    div().mb_1().h(px(1.)).w_full().bg(pal.border)
}

/// Height-capped scroll region for compact editor rows.
pub(crate) fn editor_scroll_list(
    id: &'static str,
    rows: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    crate::ui::scroll::SmoothScroll::new(id)
        .max_h(px(EDITOR_LIST_MAX_H))
        .children(rows)
}

/// A single-field "Open Application or Folder" editor, matching the Action
/// Ring editor's `path_editor`. Shared by the mouse inspector and the keyboard
/// key panel: `on_pick` receives the validated target, `on_invalid` runs when
/// Add is pressed on a blank path so the owner can set `invalid`.
pub(crate) fn application_editor(
    id_prefix: &'static str,
    input: &Entity<InputState>,
    invalid: bool,
    on_invalid: Rc<dyn Fn(&mut App)>,
    on_pick: &PickFn,
    pal: Palette,
) -> impl IntoElement {
    let on_pick = on_pick.clone();
    add_field_editor(
        id_prefix,
        "application",
        tr!("action_ring.open_application_or_folder"),
        input,
        invalid,
        move |path, window, cx| match ApplicationTarget::new(path, "") {
            Ok(target) => (on_pick)(Action::OpenApplication(target), window, cx),
            Err(_) => (on_invalid)(cx),
        },
        pal,
    )
}

/// A single-field "Run Shell Command" editor, matching the Action Ring
/// editor's Power User text field. Shared the same way as
/// [`application_editor`]: `on_pick` receives the command, `on_invalid` runs
/// when Add is pressed on a blank command so the owner can set `invalid`.
pub(crate) fn command_editor(
    id_prefix: &'static str,
    input: &Entity<InputState>,
    draft: &Entity<CommandDraft>,
    invalid: bool,
    on_invalid: Rc<dyn Fn(&mut App)>,
    on_pick: &PickFn,
    pal: Palette,
) -> impl IntoElement {
    let on_pick = on_pick.clone();
    let command_draft = draft.clone();
    add_field_editor(
        id_prefix,
        "command",
        tr!("actions.run_shell_command_heading"),
        input,
        invalid,
        move |command, window, cx| match command_draft.read(cx).action(command) {
            Some(action) => (on_pick)(action, window, cx),
            None => (on_invalid)(cx),
        },
        pal,
    )
    .child(draft.clone())
}

/// The heading, text field, Add button and inline error every single-field
/// custom-action editor shares. `on_add` receives the field's text when Add
/// is pressed. `field` names the debug selectors
/// `{id_prefix}-custom-{field}-input`, `-add` and `-error`.
fn add_field_editor(
    id_prefix: &'static str,
    field: &'static str,
    heading: gpui::SharedString,
    input: &Entity<InputState>,
    invalid: bool,
    on_add: impl Fn(String, &mut Window, &mut App) + 'static,
    pal: Palette,
) -> gpui::Div {
    let submit_input = input.clone();
    v_flex()
        .gap_1()
        .child(editor_section(heading, pal))
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(140.))
                        .debug_selector(move || format!("{id_prefix}-custom-{field}-input"))
                        .child(control_input(input).cleanable(true)),
                )
                .child(
                    Button::new(format!("{id_prefix}-custom-{field}-add"))
                        .debug_selector(move || format!("{id_prefix}-custom-{field}-add"))
                        .compact()
                        .label(if field == "application" {
                            tr!("actions.assign_open_action")
                        } else {
                            tr!("actions.assign_command")
                        })
                        .detailed_tooltip(if field == "application" {
                            tr!("actions.application_assign_tooltip")
                        } else {
                            tr!("actions.command_assign_tooltip")
                        })
                        .on_click(move |_, window, cx| {
                            let text = submit_input.read(cx).value().to_string();
                            on_add(text, window, cx);
                        }),
                ),
        )
        .when(invalid, |editor| {
            editor.child(
                div()
                    .debug_selector(move || format!("{id_prefix}-custom-{field}-error"))
                    .text_caption()
                    .text_color(rgb(0x00ef_4444))
                    .child(tr!("action_ring.custom_action_invalid_input")),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn searching_a_keyboard_key_offers_it_first_and_assigns_it(cx: &mut gpui::TestAppContext) {
        use crate::services::i18n::LOCALE_LOCK;
        use gpui::Modifiers;
        use std::cell::RefCell;

        struct PickerView(PickFn);
        impl gpui::Render for PickerView {
            fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
                v_flex().children(action_rows_with(
                    "key-search-test",
                    None,
                    "S",
                    ActionOffer::default(),
                    &self.0,
                    crate::ui::theme::palette(cx),
                ))
            }
        }
        let _locale = LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        cx.update(gpui_component::init);
        let picked = Rc::new(RefCell::new(None));
        let result = Rc::clone(&picked);
        let (_, visual) = cx.add_window_view(move |_, _| {
            PickerView(Rc::new(move |action, _, _| {
                *result.borrow_mut() = Some(action);
            }))
        });
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let expected = Action::CustomShortcut("S".parse().unwrap());
        let row = visual
            .debug_bounds("key-search-test-keyboard-query")
            .unwrap();
        let mouse = visual.debug_bounds("key-search-test-LeftClick").unwrap();
        assert!(
            row.bottom() <= mouse.top(),
            "typed key is ahead of catalog matches"
        );
        visual.simulate_click(row.center(), Modifiers::default());
        assert_eq!(*picked.borrow(), Some(expected));
        assert_eq!(
            searched_shortcut("Ctrl+S"),
            Some(Action::CustomShortcut("Ctrl+S".parse().unwrap()))
        );
        assert_eq!(searched_shortcut("not a keyboard key"), None);
        assert_eq!(searched_shortcut(""), None);
    }

    #[test]
    fn gesture_action_catalog_includes_actions_ring() {
        let actions = grouped_catalog()
            .into_iter()
            .flat_map(|(_, actions)| actions)
            .collect::<Vec<_>>();
        assert!(actions.contains(&Action::ShowActionsRing));
    }

    /// [`action_icon_path`] is exhaustive over [`Action`], so the compiler
    /// catches a missing arm — but nothing checks that the path an arm names is
    /// actually embedded, and an unembedded path renders as a blank slot with
    /// no error anywhere. [`Action::catalog`] misses the parameterised arms,
    /// which hold the only references to the keyboard and terminal glyphs.
    #[test]
    fn every_reachable_action_icon_is_embedded() {
        use gpui::AssetSource as _;
        use openlogi_core::binding::ApplicationTarget;
        use openlogi_ui::action_icons::ActionIcons;

        let combo = || "Ctrl+A".parse().expect("Ctrl+A is a valid combo");
        let uncataloged = [
            Action::SetDpiPreset(0),
            Action::CustomShortcut(combo()),
            Action::HoldShortcut(combo()),
            Action::TapKeyHoldingModifiers(combo()),
            Action::TypeText(String::new()),
            Action::RunShellCommand(String::new()),
            Action::RunPowerShellCommand(String::new()),
            Action::RunAppleScript(String::new()),
            Action::Workflow(Vec::new()),
            Action::Macro(Box::default()),
            Action::OpenApplication(
                ApplicationTarget::new("/usr/bin/true", "true").expect("non-empty path"),
            ),
        ];
        for action in Action::catalog().into_iter().chain(uncataloged) {
            let path = action_icon_path(&action);
            assert!(
                matches!(ActionIcons.load(path), Ok(Some(_))),
                "{path}, bound to {action:?}, is not embedded"
            );
        }
    }
}
