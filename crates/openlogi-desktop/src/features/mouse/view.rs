use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, RenderOnce,
    StatefulInteractiveElement as _, Styled, Subscription, Window, canvas, div, hsla, img,
    prelude::FluentBuilder as _, px, rgb,
};
use gpui_base::Button as BaseButton;
use gpui_component::{
    h_flex,
    input::{InputEvent, InputState},
    v_flex,
};
use openlogi_core::binding::{Action, ButtonId, GamingLayout, GestureDirection};

use super::force_sensing::{ForceSensingFacts, ForceSensingSlider, PressForce};
use super::geometry::{
    LABEL_H, LabelDistribution, asset_dimensions_for_png, asset_has_button_labels,
    asset_hotspots_for_png, fallback_hotspots, labels_from_hotspots, side_view_width,
};
use super::hotspots::{Hotspot, MOUSE_MODEL_SIZE, MouseControlId};
use super::inspector::{
    ActionPickerContext, BindingInspectorData, InvalidDrafts, ShortcutError, ShortcutMode,
    binding_inspector,
};
use super::leader_lines::{Geometry as LeaderGeometry, Label, paint as paint_leader_lines};
use super::macro_editor::MacroDraft;
use crate::app::{glow_canvas, keyboard_glow};
use crate::features::binding_editor::{Recording, RecordsShortcut, ShortcutRecorder};
use crate::features::keyboard::editors::{PowerUserKind, text_editor_placeholder};
use crate::features::profiles::{profile_canvas_status, profile_name};
use crate::services::assets::{GlowGeometry, ResolvedAsset};
use crate::state::{AppState, BindingLayer, DeviceKey, DeviceRecord, StateEvent};
use crate::ui::theme::{self, ACCENT, Typography as _};

fn control_label(control: MouseControlId, cx: &App) -> gpui::SharedString {
    control.button().map_or_else(
        || tr!(control.translation_key()),
        |button| {
            crate::ui::action::localized_button_label(
                button,
                AppState::try_read(cx).and_then(AppState::current_button_layout),
            )
        },
    )
}

const SIDE_GAP: f32 = 24.;
const TWO_SIDED_LABEL_MIN_W: f32 = 680.;

const CARD_EDGE_INSET: f32 = SIDE_GAP;

const HOTSPOT_DOT: f32 = 12.;
/// Left out for a device without an Actions Ring tab.
const NO_ACTIONS_RING: &[Action] = &[Action::ShowActionsRing];

/// The held actions, which only a button press can run.
fn held_actions(offer_gshift: bool) -> &'static [Action] {
    const ACTIONS: [Action; 2] = [Action::DpiShift, Action::GShift];
    if offer_gshift {
        &ACTIONS
    } else {
        &ACTIONS[..1]
    }
}

/// Vertical space occupied by the device bar, profile context, and canvas
/// padding. Normal operation no longer reserves a footer.
const MODEL_VERTICAL_RESERVE: f32 = 372.;

mod labels;
use labels::{binding_label_for_control, label_control};
/// Floor for the scaled model height. Below this the evenly-slotted side labels
/// (≈[`LABEL_H`](super::geometry::LABEL_H) each) start to overlap; the window's minimum height is sized to
/// keep the viewport above [`MODEL_VERTICAL_RESERVE`] + this.
const MODEL_MIN_H: f32 = 360.;

/// Max width the model (side gutter + image) may occupy, matching the
/// `buttons_tab` content cap so a wide keyboard image never overflows the panel.
const MODEL_CONTENT_MAX_W: f32 = 1040.;
/// Floor for the model's available width on a narrow window.
const MODEL_MIN_CONTENT_W: f32 = 160.;

/// Give the binding editor a generous share, keeping both panes usable at
/// the minimum window size and avoiding overlong fields on very wide screens.
fn inspector_width(viewport_w: f32) -> f32 {
    (viewport_w * 0.42).clamp(328., 720.)
}

struct MouseWorkspaceData<'a> {
    asset: Option<&'a ResolvedAsset>,
    layout: Option<&'static GamingLayout>,
    active: Option<MouseControlId>,
    bindings: &'a BTreeMap<ButtonId, Action>,
    gesture_maps: &'a BTreeMap<ButtonId, BTreeMap<GestureDirection, Action>>,
    glow: Option<(Arc<GlowGeometry>, Hsla)>,
    thumbwheel: bool,
    dpi_gestures: bool,
    virtual_thumbwheel: bool,
    wheel_hold: &'a BTreeSet<ButtonId>,
    editing_app: Option<String>,
    overridden: Option<&'a BTreeMap<ButtonId, Action>>,
    /// Actions the picker offers beyond the catalog.
    extra_actions: &'static [Action],
    /// Catalog actions the picker leaves out.
    hidden_actions: &'static [Action],
}

impl<'a> MouseWorkspaceData<'a> {
    fn read(cx: &'a App) -> Option<Self> {
        AppState::try_read(cx).map(|state| Self {
            asset: state
                .current_record()
                .and_then(|record| record.asset.as_ref()),
            layout: state.current_button_layout(),
            active: state
                .active_button()
                .map(MouseControlId::from_active_button),
            bindings: state.button_bindings(),
            gesture_maps: state.gesture_bindings(),
            glow: state
                .current_record()
                .and_then(|record| keyboard_glow(state, record)),
            thumbwheel: state
                .current_record()
                .and_then(|record| record.capabilities)
                .is_some_and(|capabilities| capabilities.thumbwheel),
            dpi_gestures: state
                .current_record()
                .and_then(|record| record.capabilities)
                .is_some_and(|capabilities| capabilities.dpi_gestures),
            virtual_thumbwheel: state
                .current_record()
                .and_then(|record| record.capabilities)
                .is_some_and(|capabilities| capabilities.virtual_thumbwheel),
            wheel_hold: state.wheel_hold_buttons(),
            editing_app: if state.binding_layer() == BindingLayer::GShift {
                Some(tr!("actions.g_shift").to_string())
            } else {
                state.editing_app().map(|app| profile_name(state, app))
            },
            overridden: state.editing_app_overrides(),
            extra_actions: held_actions(
                state.current_gaming_layout().is_some()
                    && state.binding_layer() == BindingLayer::Normal,
            ),
            hidden_actions: if state
                .current_record()
                .is_none_or(crate::app::offers_actions_ring)
            {
                &[]
            } else {
                NO_ACTIONS_RING
            },
        })
    }

    fn empty() -> Self {
        static NO_BINDINGS: BTreeMap<ButtonId, Action> = BTreeMap::new();
        static NO_GESTURE_MAPS: BTreeMap<ButtonId, BTreeMap<GestureDirection, Action>> =
            BTreeMap::new();
        static NO_WHEEL_HOLD: BTreeSet<ButtonId> = BTreeSet::new();
        Self {
            asset: None,
            layout: None,
            active: None,
            bindings: &NO_BINDINGS,
            gesture_maps: &NO_GESTURE_MAPS,
            glow: None,
            thumbwheel: false,
            dpi_gestures: false,
            virtual_thumbwheel: false,
            wheel_hold: &NO_WHEEL_HOLD,
            editing_app: None,
            overridden: None,
            extra_actions: &[],
            hidden_actions: &[],
        }
    }
}

/// Interactive mouse model with button hotspots.
pub struct MouseModelView {
    focus_handle: FocusHandle,
    edit_scope: MouseEditScope,
    hovered: Option<MouseControlId>,
    selected: Option<MouseControlId>,
    /// The gesture direction whose action is open in the fixed inspector.
    gesture_active_dir: Option<GestureDirection>,
    action_picker_open: bool,
    action_search: Entity<InputState>,
    pub(super) custom_shortcut_input: Entity<InputState>,
    pub(super) custom_application_input: Entity<InputState>,
    pub(super) custom_command_input: Entity<InputState>,
    pub(super) custom_command_draft: Entity<crate::features::binding_editor::CommandDraft>,
    pub(super) macro_draft: MacroDraft,
    /// Which custom editors' last "Add" attempt failed to parse, so their
    /// captions can show an inline error.
    pub(super) invalid_drafts: InvalidDrafts,
    pub(super) shortcut_mode: ShortcutMode,
    /// The custom shortcut recording in progress, if any.
    shortcut_recorder: Option<ShortcutRecorder>,
    /// The Record button's area; a recording lasts while focus stays in it.
    shortcut_record_focus: FocusHandle,
    /// The press-force slider while the haptic panel's inspector shows it.
    pub(super) force_sensing: Option<ForceSensingSlider>,
    _state_obs: Subscription,
}

/// Every coordinate that determines the destination of an assignment.
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct MouseEditScope {
    device: Option<DeviceKey>,
    profile: Option<String>,
    layer: BindingLayer,
}

impl MouseEditScope {
    pub(super) fn current(cx: &App) -> Self {
        let state = AppState::try_read(cx);
        Self {
            device: state
                .and_then(AppState::current_record)
                .map(DeviceRecord::device_key),
            profile: state.and_then(AppState::editing_app).map(str::to_owned),
            layer: state.map_or(BindingLayer::Normal, AppState::binding_layer),
        }
    }
}

impl MouseModelView {
    /// Create the mouse model view.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let action_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("actions.search_actions")));
        cx.subscribe(&action_search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let custom_shortcut_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(tr!("actions.keyboard_key_example")));
        cx.subscribe(&custom_shortcut_input, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.invalid_drafts.shortcut = None;
                cx.notify();
            }
        })
        .detach();
        let custom_application_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("action_ring.application_folder_path_or_url"))
        });
        cx.subscribe(
            &custom_application_input,
            |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.invalid_drafts.application = false;
                    cx.notify();
                }
            },
        )
        .detach();
        let custom_command_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(text_editor_placeholder(PowerUserKind::RunShellCommand))
        });
        cx.subscribe(&custom_command_input, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.invalid_drafts.command = false;
                cx.notify();
            }
        })
        .detach();
        let macro_draft = MacroDraft::new(window, cx);
        let custom_command_draft = cx.new(|_| {
            crate::features::binding_editor::CommandDraft::new(custom_command_input.clone(), None)
        });
        let state_obs = AppState::repaint_on(cx, |event| {
            matches!(
                event,
                StateEvent::ForegroundChanged
                    | StateEvent::BindingsChanged(_)
                    | StateEvent::LightingChanged(_)
                    | StateEvent::ForceSensingChanged(_)
            )
        });
        Self {
            focus_handle: cx.focus_handle(),
            edit_scope: MouseEditScope::current(cx),
            hovered: None,
            selected: None,
            gesture_active_dir: None,
            action_picker_open: false,
            action_search,
            custom_shortcut_input,
            custom_application_input,
            custom_command_input,
            custom_command_draft,
            macro_draft,
            invalid_drafts: InvalidDrafts::default(),
            shortcut_mode: ShortcutMode::Tap,
            shortcut_recorder: None,
            shortcut_record_focus: cx.focus_handle(),
            force_sensing: None,
            _state_obs: state_obs,
        }
    }

    /// Clear every custom-action draft (text and any invalid state) — called
    /// whenever the picker opens for a new target, so a shortcut, application
    /// or command typed for one button doesn't reappear for another.
    pub(super) fn clear_custom_action_drafts(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.custom_shortcut_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.custom_application_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.custom_command_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.custom_command_draft.update(cx, |draft, cx| {
            draft.reset(window, cx);
        });
        self.macro_draft.clear(window, cx);
        self.invalid_drafts = InvalidDrafts::default();
        self.shortcut_mode = ShortcutMode::Tap;
        self.shortcut_recorder = None;
    }

    /// Set (or clear, with `None`) the activated gesture direction. Callers must
    /// `cx.notify()` to re-render.
    pub(crate) fn set_gesture_selected_dir(&mut self, dir: Option<GestureDirection>) {
        self.gesture_active_dir = dir;
        self.action_picker_open = false;
        self.shortcut_recorder = None;
    }

    pub(super) fn toggle_action_picker(&mut self) {
        self.action_picker_open = !self.action_picker_open;
        self.shortcut_recorder = None;
    }

    pub(super) fn close_action_picker(&mut self) {
        self.action_picker_open = false;
        self.shortcut_recorder = None;
    }

    pub(super) fn show_action_picker(&mut self) {
        self.action_picker_open = true;
    }

    /// Start recording the custom shortcut, or stop the recording in
    /// progress.
    pub(super) fn toggle_shortcut_recording(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shortcut_recorder.take().is_none() {
            let area = self.shortcut_record_focus.clone();
            self.shortcut_recorder = Some(ShortcutRecorder::start(&area, window, cx));
        }
        cx.notify();
    }

    /// The action picker's state, as the inspector reads it.
    fn action_picker_context<'a>(&'a self, view: &'a Entity<Self>) -> ActionPickerContext<'a> {
        ActionPickerContext {
            open: self.action_picker_open,
            selected: self.selected,
            direction: self.gesture_active_dir,
            search: &self.action_search,
            shortcut_input: &self.custom_shortcut_input,
            application_input: &self.custom_application_input,
            command_input: &self.custom_command_input,
            command_draft: &self.custom_command_draft,
            macro_draft: &self.macro_draft,
            shortcut_mode: self.shortcut_mode,
            shortcut_recording: self.shortcut_recorder.is_some(),
            shortcut_record_focus: &self.shortcut_record_focus,
            invalid: self.invalid_drafts,
            view,
        }
    }

    fn reset_for_scope(
        &mut self,
        scope: MouseEditScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit_scope == scope {
            return;
        }
        if self.edit_scope.device != scope.device {
            self.hovered = None;
            self.selected = None;
        }
        self.edit_scope = scope;
        self.gesture_active_dir = None;
        self.action_picker_open = false;
        self.shortcut_recorder = None;
        self.clear_custom_action_drafts(window, cx);
    }

    pub(super) fn select(
        &mut self,
        control: MouseControlId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected != Some(control) || !self.action_picker_open {
            self.selected = Some(control);
            self.gesture_active_dir = None;
            self.clear_custom_action_drafts(window, cx);
            self.action_search
                .update(cx, |search, cx| search.set_value("", window, cx));
            if let MouseControlId::Button(button) = control
                && let Some(Action::Macro(sequence)) = AppState::try_read(cx)
                    .and_then(|state| state.button_bindings().get(&button))
                    .cloned()
            {
                self.macro_draft.seed(&sequence, window, cx);
            }
            self.action_picker_open = true;
        }
    }
}

impl Focusable for MouseModelView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn set_control_hovered(
    view: &Entity<MouseModelView>,
    control: MouseControlId,
    hovered: bool,
    cx: &mut App,
) {
    view.update(cx, |this, cx| {
        if hovered {
            this.hovered = Some(control);
        } else if this.hovered == Some(control) {
            this.hovered = None;
        }
        cx.notify();
    });
}

impl MouseModelView {
    /// Re-stamp every action-picker input's placeholder after a language
    /// switch, split out of `render` to keep it under clippy's line budget.
    fn localize_action_picker_inputs(&self, window: &mut Window, cx: &mut Context<Self>) {
        crate::ui::components::localize_placeholder(
            &self.action_search,
            tr!("actions.search_actions"),
            window,
            cx,
        );
        crate::ui::components::localize_placeholder(
            &self.custom_shortcut_input,
            tr!("actions.keyboard_key_example"),
            window,
            cx,
        );
        crate::ui::components::localize_placeholder(
            &self.custom_application_input,
            tr!("action_ring.application_folder_path_or_url"),
            window,
            cx,
        );
        self.macro_draft.localize_placeholders(window, cx);
    }

    /// Follow a device change, then keep the press-force slider in step with
    /// the selection it may have cleared. Runs before `render` reads the
    /// state for the frame, since the slider sync needs the context mutably.
    fn sync_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ForceSensingFacts> {
        let scope = MouseEditScope::current(cx);
        self.reset_for_scope(scope, window, cx);
        let facts = ForceSensingFacts::current(self.selected, cx);
        ForceSensingSlider::sync(&mut self.force_sensing, facts.as_ref(), window, cx);
        facts
    }
}

impl RecordsShortcut for MouseModelView {
    /// A recorded chord fills the shortcut field, and Add commits it in the
    /// chosen mode. A key the recorder cannot turn into a chord shows the
    /// field's error and leaves the field as it was.
    fn shortcut_recorded(
        &mut self,
        recording: Recording,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shortcut_recorder = None;
        match recording {
            Recording::Chord(combo) => {
                self.custom_shortcut_input.update(cx, |input, cx| {
                    input.set_value(combo.rendered_label(), window, cx);
                });
                self.invalid_drafts.shortcut = None;
            }
            Recording::Cancelled => {}
            Recording::Unsupported => {
                self.invalid_drafts.shortcut = Some(ShortcutError::Unrecognized);
            }
            Recording::ShiftedSymbol => {
                self.invalid_drafts.shortcut = Some(ShortcutError::ShiftedSymbol);
            }
        }
        cx.notify();
    }
}

fn displayed_gestures(
    gesture_maps: &BTreeMap<ButtonId, BTreeMap<GestureDirection, Action>>,
    editing_app: Option<&str>,
    overridden: Option<&BTreeMap<ButtonId, Action>>,
) -> Vec<ButtonId> {
    gesture_maps
        .keys()
        .copied()
        .filter(|button| {
            editing_app.is_none()
                || !overridden.is_some_and(|overrides| overrides.contains_key(button))
        })
        .collect()
}

impl Render for MouseModelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.localize_action_picker_inputs(window, cx);
        let force_sensing = self.sync_selection(window, cx);
        let MouseWorkspaceData {
            asset,
            layout,
            active,
            bindings,
            gesture_maps,
            glow,
            thumbwheel,
            dpi_gestures,
            virtual_thumbwheel,
            wheel_hold,
            editing_app,
            overridden,
            extra_actions,
            hidden_actions,
        } = MouseWorkspaceData::read(cx).unwrap_or_else(MouseWorkspaceData::empty);

        let gesture_buttons = displayed_gestures(gesture_maps, editing_app.as_deref(), overridden);

        let viewport_h = f32::from(window.viewport_size().height);
        let viewport_w = f32::from(window.viewport_size().width);
        let ModelLayout {
            canvas_w,
            canvas_h,
            mouse_top,
            mouse_left,
            mouse_w,
            mouse_h,
            label_w,
            hotspots,
            labels,
            overlay_gshift,
        } = model_layout(asset, layout, viewport_w, viewport_h, thumbwheel);

        let highlight = self.hovered.or(active).or(self.selected);
        let view = cx.entity();
        let hovered = self.hovered;
        let profile_status = profile_canvas_status(cx);

        let hotspots_outer = hotspots.clone();
        let labels_outer = labels.clone();
        let leader_canvas = leader_canvas(hotspots, labels, highlight, mouse_left, mouse_w);
        let breathing_art = breathing_art(asset, mouse_left, mouse_w, mouse_h, glow);
        let model = ModelRect {
            left: mouse_left,
            width: mouse_w,
            height: mouse_h,
            label_width: label_w,
        };
        let hotspots_layer = hotspots_layer(
            &hotspots_outer,
            model,
            hovered,
            active,
            self.selected,
            &view,
        );
        let model_band = div()
            .child(breathing_art)
            .child(leader_canvas)
            .children(labels_outer.iter().enumerate().map(|(idx, label)| {
                let binding = binding_label_for_control(label.id, bindings, &gesture_buttons);
                label_control(
                    idx,
                    *label,
                    binding,
                    hovered == Some(label.id) || active == Some(label.id),
                    model,
                    self.selected == Some(label.id),
                    &view,
                )
            }))
            .child(hotspots_layer);
        let canvas = model_canvas(model_band, canvas_w, canvas_h, mouse_top, mouse_h);

        let inspector = binding_inspector(
            BindingInspectorData {
                selected: self.selected,
                gesture_direction: self.gesture_active_dir,
                bindings,
                layout,
                gesture_maps,
                dpi_gestures,
                virtual_thumbwheel,
                wheel_hold,
                editing_app: editing_app.as_deref(),
                overridden,
                press_force: PressForce::of(force_sensing.as_ref(), self.force_sensing.as_ref()),
                extra_actions,
                hidden_actions,
            },
            self.action_picker_context(&view),
            cx,
        );
        workspace_layout(
            canvas,
            profile_status,
            inspector,
            inspector_width(viewport_w),
            overlay_gshift,
            &self.focus_handle,
            cx,
        )
    }
}

/// The canvas around `band`, which holds the model and its cards laid out
/// against the model's own `mouse_h`. The cards a crowded side pushes past
/// the model's top or bottom still land inside the canvas, which grows to
/// `canvas_h` to hold them, with the model `mouse_top` down from its top.
fn model_canvas(
    band: gpui::Div,
    canvas_w: f32,
    canvas_h: f32,
    mouse_top: f32,
    mouse_h: f32,
) -> gpui::Div {
    div()
        .relative()
        .w(px(canvas_w))
        .h(px(canvas_h))
        .flex_none()
        .debug_selector(|| "mouse-canvas".into())
        .child(
            band.absolute()
                .top(px(mouse_top))
                .left(px(0.))
                .w(px(canvas_w))
                .h(px(mouse_h)),
        )
}

fn workspace_layout(
    canvas: impl IntoElement,
    profile_status: Option<gpui::Div>,
    inspector: impl IntoElement,
    inspector_w: f32,
    overlay_gshift: bool,
    focus_handle: &FocusHandle,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .w_full()
        .items_stretch()
        .tab_group()
        .track_focus(focus_handle)
        .child(
            h_flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .items_stretch()
                .child(
                    div()
                        .w(px(inspector_w))
                        .flex_none()
                        .h_full()
                        .min_h_0()
                        .child(inspector),
                )
                .child(
                    v_flex()
                        .debug_selector(|| "mouse-preview".into())
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_hidden()
                        .children(profile_status)
                        .child(
                            crate::ui::scroll::SmoothScroll::new("mouse-model-scroll")
                                .debug_selector(|| "mouse-model-scroll".into())
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .flex()
                                .items_start()
                                .justify_center()
                                .p_4()
                                .child(canvas),
                        )
                        .children(gshift_layer_bar(overlay_gshift, cx)),
                ),
        )
        .children(crate::features::profiles::profile_switching_footer(cx))
}

/// In a two-sided layout the latch shares the centre gutter with side cards.
/// A narrow, single-column preview needs a separate strip below its scroller.
fn gshift_layer_bar(overlay: bool, cx: &App) -> Option<impl IntoElement> {
    let state = AppState::try_read(cx)?;
    if state.current_gaming_layout().is_none() || !state.current_device_is_persistent() {
        return None;
    }
    let latched = state.gshift_latched();
    let applied = state.apply_gshift_while_editing();
    let pal = theme::palette(cx);
    Some(
        v_flex()
            .debug_selector(|| "gshift-layer-bar".into())
            .when(overlay, |bar| bar.absolute().bottom_0())
            .w_full()
            .min_h(px(116.))
            .flex_none()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .h(px(18.))
                    .text_body()
                    .text_color(if latched {
                        pal.text_muted
                    } else {
                        pal.text_primary
                    })
                    .child(tr!("actions.g_shift_normal")),
            )
            .child(
                div()
                    .id("gshift-layer-help")
                    .debug_selector(|| "gshift-switch-slot".into())
                    .tooltip(|window, cx| {
                        crate::ui::components::detailed_tooltip(
                            tr!("actions.g_shift_edit_tooltip"),
                            window,
                            cx,
                        )
                    })
                    .relative()
                    .child(
                        gpui_component::switch::Switch::new("gshift-layer-switch")
                            .checked(latched)
                            .on_click(|checked, _, cx| {
                                AppState::apply(cx, |state| state.set_gshift_latched(*checked));
                            }),
                    )
                    .when(latched, |slot| slot.child(apply_gshift_switch(applied))),
            )
            .child(
                div()
                    .h(px(18.))
                    .text_body()
                    .text_color(if latched {
                        pal.text_primary
                    } else {
                        pal.text_muted
                    })
                    .child(tr!("actions.g_shift")),
            )
            .child(
                div()
                    .text_caption()
                    .text_center()
                    .when(overlay, |caption| caption.max_w(px(180.)))
                    .text_color(pal.text_muted)
                    .child(if applied {
                        tr!("actions.g_shift_keyboard_off")
                    } else {
                        tr!("actions.g_shift_edit_only")
                    }),
            ),
    )
}

struct ModelLayout {
    overlay_gshift: bool,
    canvas_w: f32,
    /// The model's height plus whatever the cards reach past it.
    canvas_h: f32,
    /// How far the model sits below the canvas top: how far the cards reach
    /// above it.
    mouse_top: f32,
    mouse_left: f32,
    mouse_w: f32,
    mouse_h: f32,
    label_w: f32,
    hotspots: Vec<Hotspot>,
    labels: Vec<Label>,
}

fn apply_gshift_switch(applied: bool) -> impl IntoElement {
    h_flex()
        .debug_selector(|| "apply-gshift-editing".into())
        .absolute()
        .left_full()
        .ml_4()
        .gap_2()
        .items_center()
        .child(
            div()
                .id("apply-gshift-help")
                .tooltip(|window, cx| {
                    crate::ui::components::detailed_tooltip(
                        tr!("actions.apply_g_shift_tooltip"),
                        window,
                        cx,
                    )
                })
                .debug_selector(|| "apply-gshift-switch".into())
                .child(
                    gpui_component::switch::Switch::new("apply-gshift-editing")
                        .checked(applied)
                        .on_click(|enabled, _, cx| {
                            AppState::apply(cx, |state| {
                                state.set_apply_gshift_while_editing(*enabled)
                            });
                        }),
                ),
        )
        .child(
            div()
                .w(px(150.))
                .text_caption()
                .child(tr!("actions.apply_g_shift_editing")),
        )
}

/// Scale the model to fit the content area in both axes. A tall mouse is bound
/// by the viewport height; a wide keyboard is bound by the available width and
/// drops the label gutter so it remains centred.
fn model_layout(
    asset: Option<&ResolvedAsset>,
    layout: Option<&GamingLayout>,
    viewport_w: f32,
    viewport_h: f32,
    thumbwheel: bool,
) -> ModelLayout {
    let target_h = (viewport_h - MODEL_VERTICAL_RESERVE).clamp(MODEL_MIN_H, MOUSE_MODEL_SIZE.1);
    // The cards may use the tab's whole height, not just the model's: a
    // narrow window shrinks the model by its width and leaves room beside it.
    let room_h = (viewport_h - MODEL_VERTICAL_RESERVE).max(MODEL_MIN_H);
    let content_w = (viewport_w - inspector_width(viewport_w) - 48.)
        .clamp(MODEL_MIN_CONTENT_W, MODEL_CONTENT_MAX_W);
    let has_labels =
        asset.is_none_or(|asset| asset_has_button_labels(asset, layout)) && content_w >= 400.;
    let label_distribution = if has_labels && content_w >= TWO_SIDED_LABEL_MIN_W {
        LabelDistribution::BothSides
    } else {
        LabelDistribution::LeftOnly
    };
    let label_w = (content_w * 0.28).clamp(220., 320.);
    let left_gutter = if has_labels { label_w + SIDE_GAP } else { 0. };
    let right_gutter = if label_distribution == LabelDistribution::BothSides {
        label_w + SIDE_GAP
    } else {
        0.
    };
    let max_image_w = (content_w - left_gutter - right_gutter).max(MODEL_MIN_CONTENT_W / 2.);
    let (mouse_w, mouse_h, hotspots, mut labels) = scaled_model(
        asset,
        layout,
        target_h,
        room_h,
        max_image_w,
        thumbwheel,
        label_distribution,
    );
    if !has_labels {
        labels.clear();
    }
    let cards_top = labels
        .iter()
        .map(|label| label.y - LABEL_H / 2.)
        .fold(0., f32::min);
    let cards_bottom = labels
        .iter()
        .map(|label| label.y + LABEL_H / 2.)
        .fold(mouse_h, f32::max);

    ModelLayout {
        canvas_w: left_gutter + mouse_w + right_gutter,
        canvas_h: cards_bottom - cards_top,
        mouse_top: -cards_top,
        mouse_left: left_gutter,
        mouse_w,
        mouse_h,
        label_w,
        hotspots,
        labels,
        overlay_gshift: label_distribution == LabelDistribution::BothSides,
    }
}

/// Model geometry fit inside a `max_w` × `target_h` box. With a real asset the
/// hotspots and labels are recomputed from the scaled dimensions; the synthetic
/// silhouette's authored coordinates are scaled by the same factor. The labels
/// may use `room_h` (see [`labels_from_hotspots`]). Returns
/// `(mouse_w, mouse_h, hotspots, labels)`.
fn scaled_model(
    asset: Option<&ResolvedAsset>,
    layout: Option<&GamingLayout>,
    target_h: f32,
    room_h: f32,
    max_w: f32,
    thumbwheel: bool,
    label_distribution: LabelDistribution,
) -> (f32, f32, Vec<Hotspot>, Vec<Label>) {
    if let Some(a) = asset {
        let (w, h) = asset_dimensions_for_png(a, target_h, max_w);
        let hotspots = asset_hotspots_for_png(a, layout, w, h);
        let labels = labels_from_hotspots(&hotspots, w, h, room_h, label_distribution);
        (w, h, hotspots, labels)
    } else {
        let scale = (target_h / MOUSE_MODEL_SIZE.1).min(max_w / MOUSE_MODEL_SIZE.0);
        let (w, h) = (MOUSE_MODEL_SIZE.0 * scale, MOUSE_MODEL_SIZE.1 * scale);
        let hotspots: Vec<Hotspot> = fallback_hotspots(layout, thumbwheel)
            .into_iter()
            .map(|hs| Hotspot {
                x: hs.x * scale,
                y: hs.y * scale,
                w: hs.w * scale,
                h: hs.h * scale,
                ..hs
            })
            .collect();
        // Placed at the scaled size, like an asset's: the cards keep their
        // height however small the silhouette gets.
        let labels = labels_from_hotspots(&hotspots, w, h, room_h, label_distribution);
        (w, h, hotspots, labels)
    }
}

fn leader_canvas(
    hotspots: Vec<Hotspot>,
    labels: Vec<Label>,
    highlight: Option<MouseControlId>,
    mouse_left: f32,
    mouse_w: f32,
) -> impl IntoElement {
    canvas(
        move |_bounds, _, _| (hotspots, labels, highlight),
        move |bounds, payload, window, _app| {
            let (hotspots, labels, highlight) = payload;
            paint_leader_lines(
                bounds,
                LeaderGeometry {
                    mouse_origin: gpui::point(px(mouse_left), px(0.)),
                    mouse_w,
                    card_edge_inset: CARD_EDGE_INSET,
                },
                &hotspots,
                &labels,
                highlight,
                window,
            );
        },
    )
    .size_full()
}

fn breathing_art(
    asset: Option<&ResolvedAsset>,
    mouse_left: f32,
    mouse_w: f32,
    mouse_h: f32,
    glow: Option<(Arc<GlowGeometry>, Hsla)>,
) -> impl IntoElement {
    let device_art: AnyElement = match asset {
        Some(a) => {
            let side_w = side_view_width(a, mouse_h);
            h_flex()
                .when_some(a.side_view.as_ref(), |this, side| {
                    this.child(img(side.image_path.clone()).w(px(side_w)).h(px(mouse_h)))
                })
                .child(
                    img(a.image_path.clone())
                        .w(px(mouse_w - side_w))
                        .h(px(mouse_h)),
                )
                .into_any_element()
        }
        None => Silhouette {
            w: mouse_w,
            h: mouse_h,
        }
        .into_any_element(),
    };
    div()
        .absolute()
        .left(px(mouse_left))
        .top(px(0.))
        .w(px(mouse_w))
        .h(px(mouse_h))
        // Paint the keyboard's RGB *behind* the render so the opaque keys occlude
        // it and the colour only reads through the inter-key gaps — light from
        // behind, not specks on top. Same effect as the home gallery, scaled to
        // this render with no pre-baked PNG (#272).
        .when_some(glow, |this, (geom, color)| {
            this.child(glow_canvas(geom, color))
        })
        .child(device_art)
}

#[derive(Clone, Copy)]
struct ModelRect {
    left: f32,
    width: f32,
    height: f32,
    label_width: f32,
}

fn hotspots_layer(
    hotspots: &[Hotspot],
    model: ModelRect,
    hovered: Option<MouseControlId>,
    active: Option<MouseControlId>,
    selected: Option<MouseControlId>,
    view: &Entity<MouseModelView>,
) -> impl IntoElement {
    div()
        .absolute()
        .left(px(model.left))
        .top(px(0.))
        .w(px(model.width))
        .h(px(model.height))
        .children(hotspots.iter().enumerate().map(|(idx, hotspot)| {
            hotspot_control(
                idx,
                *hotspot,
                hovered,
                active,
                selected == Some(hotspot.id),
                view,
            )
        }))
}

/// Shape-based silhouette used when no asset is cached for the device.
///
/// Its `rounded_*` values are illustration proportions — the body shell and the
/// two drawn side buttons — not UI chrome, so they stay fixed rather than
/// tracking the `Palette` radius tokens the way real cards and controls do.
#[derive(IntoElement)]
struct Silhouette {
    w: f32,
    h: f32,
}

impl RenderOnce for Silhouette {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { w, h } = self;
        let pal = theme::palette(cx);
        div()
            .absolute()
            .inset_0()
            .w(px(w))
            .h(px(h))
            .rounded_3xl()
            .border_1()
            .border_color(pal.text_muted)
            .bg(pal.muted)
            .child(
                div()
                    .absolute()
                    .left(px(w / 2. - 14.))
                    .top(px(90.))
                    .w(px(28.))
                    .h(px(110.))
                    .rounded_md()
                    .bg(hsla(0., 0., 0.25, 1.0)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(w / 2.))
                    .top(px(20.))
                    .w(px(1.))
                    .h(px(240.))
                    .bg(pal.border),
            )
            .child(
                div()
                    .absolute()
                    .left(px(8.))
                    .top(px(210.))
                    .w(px(34.))
                    .h(px(150.))
                    .rounded_md()
                    .bg(hsla(0., 0., 0.25, 1.0)),
            )
    }
}

fn hotspot_control(
    idx: usize,
    hotspot: Hotspot,
    hovered: Option<MouseControlId>,
    active: Option<MouseControlId>,
    selected: bool,
    view: &Entity<MouseModelView>,
) -> gpui::Div {
    let view = view.clone();
    let trigger = HotspotTrigger {
        id: ("hotspot-trigger", idx).into(),
        hotspot,
        hovered: hovered == Some(hotspot.id) || active == Some(hotspot.id),
        view,
        selected,
    };
    div()
        .absolute()
        .left(px(hotspot.x))
        .top(px(hotspot.y))
        .w(px(hotspot.w))
        .h(px(hotspot.h))
        .child(trigger)
}

#[derive(IntoElement)]
struct HotspotTrigger {
    id: ElementId,
    hotspot: Hotspot,
    hovered: bool,
    view: Entity<MouseModelView>,
    selected: bool,
}

impl RenderOnce for HotspotTrigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let highlighted = self.hovered || self.selected;
        let selected = self.selected;
        let view = self.view;
        let click_view = view.clone();
        let hotspot = self.hotspot;
        let btn = hotspot.id;
        let data = MouseWorkspaceData::read(cx).unwrap_or_else(MouseWorkspaceData::empty);
        let gestures = displayed_gestures(
            data.gesture_maps,
            data.editing_app.as_deref(),
            data.overridden,
        );
        let description = binding_label_for_control(btn, data.bindings, &gestures).description();
        let name = control_label(btn, cx);
        let tooltip = format!("{name}: {description}");

        BaseButton::new(self.id)
            .selected(selected)
            .accessibility_label(tr!(
                "actions.bind_control",
                name => name
            ))
            .aria_selected(selected)
            .aria_description(description)
            .tooltip(move |window, cx| {
                crate::ui::components::detailed_tooltip(tooltip.clone(), window, cx)
            })
            .flex()
            .items_center()
            .justify_center()
            .w(px(hotspot.w))
            .h(px(hotspot.h))
            .child(
                div()
                    .w(px(HOTSPOT_DOT))
                    .h(px(HOTSPOT_DOT))
                    .rounded_full()
                    .border_1()
                    .border_color(if highlighted {
                        gpui::Hsla::from(rgb(ACCENT))
                    } else {
                        hsla(0., 0., 0.95, 0.85)
                    })
                    .bg(if highlighted {
                        gpui::Hsla::from(rgb(ACCENT))
                    } else {
                        hsla(0., 0., 0.18, 0.85)
                    }),
            )
            .focus_visible(|style| style.rounded_full().border_2().border_color(rgb(ACCENT)))
            .on_click(move |_event, window, cx| {
                click_view.update(cx, |this, cx| {
                    this.select(btn, window, cx);
                    cx.notify();
                });
            })
            .on_hover(move |hovered, _window, cx| {
                set_control_hovered(&view, btn, *hovered, cx);
            })
    }
}

#[cfg(test)]
mod tests;
