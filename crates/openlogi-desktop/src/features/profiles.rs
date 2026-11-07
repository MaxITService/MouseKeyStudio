//! Per-application profiles shared by device feature editors.

mod applications;
mod catalog;
mod executable;
mod picker;
mod selector;
mod shell;

use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use gpui::{App, Entity, ParentElement, Styled, Window, div, prelude::FluentBuilder as _};
use gpui_component::{WindowExt as _, button::ButtonVariant, dialog::DialogButtonProps, h_flex};
use openlogi_core::app::{AppAliases, profile_key_for};

pub(crate) use self::catalog::{AppCatalogPicker, ProfileIconCache};
use self::shell::ProfileScopeShell;
use crate::state::{AppState, DeviceKey};
use crate::ui::theme::{self, Typography as _};

#[derive(Clone)]
pub(super) struct ProfileChoice {
    /// The profile key: an application identifier, or `app:<alias>`.
    pub(super) app: String,
    pub(super) name: String,
    /// The application identifier whose icon stands for the profile: the key
    /// itself, or the first identifier its alias lists on this platform.
    pub(super) icon: String,
    pub(super) persisted: bool,
}

impl ProfileChoice {
    /// A not-yet-saved profile for an application keyed by its identifier.
    pub(super) fn application(app: String, name: String) -> Self {
        Self {
            icon: app.clone(),
            app,
            name,
            persisted: false,
        }
    }
}

/// Which applications already run under one of an editor's saved profiles,
/// matched the way the agent matches them â€” so the picker never offers an
/// application whose profile tab is already the one that applies to it.
pub(super) struct ProfileCoverage {
    /// The saved profiles, by key.
    profiles: BTreeMap<String, ProfileChoice>,
    aliases: AppAliases,
}

impl ProfileCoverage {
    fn new(profiles: &[ProfileChoice], aliases: &AppAliases) -> Self {
        Self {
            profiles: profiles
                .iter()
                .map(|profile| (profile.app.clone(), profile.clone()))
                .collect(),
            aliases: aliases.clone(),
        }
    }

    fn covers(&self, app: &str) -> bool {
        profile_key_for(&self.profiles, &self.aliases, app).is_some()
    }
}

pub(super) enum CatalogPresentation {
    Loading,
    Ready(Vec<ProfileChoice>),
    Failed,
}

pub(super) struct AddAppChoices {
    pub(super) recent: Vec<ProfileChoice>,
    pub(super) catalog: CatalogPresentation,
}

pub(super) struct ProfileScopeModel {
    pub(super) editing_app: Option<String>,
    pub(super) profiles: Vec<ProfileChoice>,
    pub(super) choices: AddAppChoices,
}

type SelectProfile = dyn Fn(Option<String>, &mut App);
type ChangeProfile = dyn Fn(ProfileChoice, &mut Window, &mut App);

/// Feature-owned behavior invoked by the profile selector shell.
#[derive(Clone)]
pub(super) struct ProfileScopeActions {
    select: Rc<SelectProfile>,
    reset: Rc<ChangeProfile>,
    remove: Rc<ChangeProfile>,
    remove_all: Rc<ChangeProfile>,
}

impl ProfileScopeActions {
    fn new(
        select: impl Fn(Option<String>, &mut App) + 'static,
        reset: impl Fn(ProfileChoice, &mut Window, &mut App) + 'static,
        remove: impl Fn(ProfileChoice, &mut Window, &mut App) + 'static,
        remove_all: impl Fn(ProfileChoice, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            select: Rc::new(select),
            reset: Rc::new(reset),
            remove: Rc::new(remove),
            remove_all: Rc::new(remove_all),
        }
    }

    pub(super) fn select(&self, app: Option<String>, cx: &mut App) {
        (self.select)(app, cx);
    }

    pub(super) fn reset(&self, profile: ProfileChoice, window: &mut Window, cx: &mut App) {
        (self.reset)(profile, window, cx);
    }

    pub(super) fn remove(&self, profile: ProfileChoice, window: &mut Window, cx: &mut App) {
        (self.remove)(profile, window, cx);
    }

    pub(super) fn remove_all(&self, profile: ProfileChoice, window: &mut Window, cx: &mut App) {
        (self.remove_all)(profile, window, cx);
    }
}

/// Build the Buttons workspace's profile selector.
pub(crate) fn button_profile_scope_bar(
    icons: &ProfileIconCache,
    catalog: &Entity<AppCatalogPicker>,
    cx: &mut App,
) -> Option<ProfileScopeShell> {
    let state = AppState::try_read(cx)?;
    if !state.current_device_is_persistent() {
        return None;
    }
    let key = state.current_record()?.device_key();
    let reset_key = key.clone();
    let remove_all_key = key.clone();
    let editing_app = state.editing_app().map(str::to_string);
    let saved: Vec<&str> = state.app_profiles().map(|(app, _)| app).collect();
    let (profiles, coverage) = scope_profiles(state, &saved, editing_app.as_deref());
    let recent_apps: Vec<(String, String)> = state
        .recent_apps()
        .map(|(app, name)| (app.to_string(), name.to_string()))
        .collect();
    let model = profile_scope_model(editing_app, profiles, &coverage, &recent_apps, catalog, cx);
    let actions = ProfileScopeActions::new(
        |app, cx| {
            AppState::apply(cx, |state| {
                let app = resolve_selection(
                    app,
                    state.app_profiles().map(|(app, _)| app),
                    state.app_aliases(),
                );
                state.set_editing_app(app)
            });
        },
        move |profile, window, cx| {
            ProfileCommand::ResetButtons.confirm(&reset_key, &profile, window, cx);
        },
        move |profile, window, cx| {
            ProfileCommand::RemoveButtons.confirm(&key, &profile, window, cx);
        },
        move |profile, window, cx| {
            ProfileCommand::RemoveAll.confirm(&remove_all_key, &profile, window, cx);
        },
    );

    Some(ProfileScopeShell::new(
        "button-profile",
        model,
        catalog.clone(),
        icons.clone(),
        actions,
    ))
}

/// Build the Actions Ring workspace's independent profile selector.
pub(crate) fn action_ring_profile_scope_bar(
    icons: &ProfileIconCache,
    catalog: &Entity<AppCatalogPicker>,
    cx: &mut App,
) -> Option<ProfileScopeShell> {
    let state = AppState::try_read(cx)?;
    if !state.current_device_is_persistent() {
        return None;
    }
    let key = state.current_record()?.device_key();
    let reset_key = key.clone();
    let remove_all_key = key.clone();
    let editing_app = state.editing_action_ring_app().map(str::to_string);
    let ring = state.current_action_ring();
    let saved: Vec<&str> = ring.per_app.keys().map(String::as_str).collect();
    let (profiles, coverage) = scope_profiles(state, &saved, editing_app.as_deref());
    let recent_apps: Vec<(String, String)> = state
        .recent_apps()
        .map(|(app, name)| (app.to_string(), name.to_string()))
        .collect();
    let model = profile_scope_model(editing_app, profiles, &coverage, &recent_apps, catalog, cx);
    let actions = ProfileScopeActions::new(
        |app, cx| {
            AppState::apply(cx, |state| {
                let ring = state.current_action_ring();
                let app = resolve_selection(
                    app,
                    ring.per_app.keys().map(String::as_str),
                    state.app_aliases(),
                );
                state.set_editing_action_ring_app(app)
            });
        },
        move |profile, window, cx| {
            ProfileCommand::ResetRing.confirm(&reset_key, &profile, window, cx);
        },
        move |profile, window, cx| {
            ProfileCommand::RemoveRing.confirm(&key, &profile, window, cx);
        },
        move |profile, window, cx| {
            ProfileCommand::RemoveAll.confirm(&remove_all_key, &profile, window, cx);
        },
    );

    Some(ProfileScopeShell::new(
        "action-ring-profile",
        model,
        catalog.clone(),
        icons.clone(),
        actions,
    ))
}

fn resolve_selection<'a>(
    app: Option<String>,
    saved: impl Iterator<Item = &'a str>,
    aliases: &AppAliases,
) -> Option<String> {
    app.map(|app| {
        let profiles = saved.map(|key| (key.to_string(), key)).collect();
        profile_key_for(&profiles, aliases, &app).map_or_else(|| app.clone(), str::to_string)
    })
}

/// An editor's profile tabs â€” its `saved` profile keys, plus the unsaved
/// draft for `editing_app` when it has none â€” and what the saved ones cover.
fn scope_profiles(
    state: &AppState,
    saved: &[&str],
    editing_app: Option<&str>,
) -> (Vec<ProfileChoice>, ProfileCoverage) {
    let mut profiles: Vec<ProfileChoice> = saved
        .iter()
        .map(|app| profile_choice(state, app, true))
        .collect();
    let coverage = ProfileCoverage::new(&profiles, state.app_aliases());
    if let Some(app) = editing_app
        && !saved.contains(&app)
    {
        profiles.push(profile_choice(state, app, false));
    }
    (profiles, coverage)
}

/// The tab for the profile keyed `key`, named and illustrated the way the user
/// knows the application.
fn profile_choice(state: &AppState, key: &str, persisted: bool) -> ProfileChoice {
    let icon = state
        .app_aliases()
        .for_profile_key(key)
        .and_then(|(_, alias)| alias.identifiers_here().first())
        .map_or(key, String::as_str);
    ProfileChoice {
        app: key.to_string(),
        name: profile_name(state, key),
        icon: icon.to_string(),
        persisted,
    }
}

/// What the UI calls the profile keyed `key` — the shared
/// [`openlogi_core::app::profile_display_name`] over the applications the agent
/// reported this session, so the tray and the window agree.
pub(crate) fn profile_name(state: &AppState, key: &str) -> String {
    state.profile_name(key)
}

fn profile_scope_model(
    editing_app: Option<String>,
    mut profiles: Vec<ProfileChoice>,
    coverage: &ProfileCoverage,
    recent_apps: &[(String, String)],
    catalog: &Entity<AppCatalogPicker>,
    cx: &mut App,
) -> ProfileScopeModel {
    profiles.sort_by_key(|profile| profile.name.to_lowercase());

    let persisted_ids: HashSet<String> = profiles
        .iter()
        .filter(|profile| profile.persisted)
        .map(|profile| profile.app.clone())
        .collect();
    let observed_ids: HashSet<String> = recent_apps.iter().map(|(app, _)| app.clone()).collect();
    let available_recent: Vec<ProfileChoice> = recent_apps
        .iter()
        .filter(|(app, _)| !coverage.covers(app) && editing_app.as_deref() != Some(app.as_str()))
        .map(|(app, name)| ProfileChoice::application(app.clone(), name.clone()))
        .collect();
    let mut unavailable = persisted_ids;
    unavailable.extend(observed_ids.iter().cloned());
    unavailable.extend(editing_app.iter().cloned());
    catalog.update(cx, |picker, cx| {
        for profile in &profiles {
            picker.ensure_icon(&profile.icon, cx);
        }
    });
    let catalog_presentation = catalog.read(cx).available_profiles(&observed_ids, |app| {
        unavailable.contains(app) || coverage.covers(app)
    });
    let choices = AddAppChoices {
        recent: available_recent,
        catalog: catalog_presentation,
    };
    ProfileScopeModel {
        editing_app,
        profiles,
        choices,
    }
}

/// Automatic runtime profile switching, with an explicit locked-profile caption.
pub(crate) fn profile_switching_footer(cx: &App) -> Option<gpui::Div> {
    use crate::ui::components::{Toggle, toggle_row};
    use gpui::{InteractiveElement as _, StatefulInteractiveElement as _};
    use gpui_component::Selectable as _;
    let state = AppState::try_read(cx)?;
    if !state.current_device_is_persistent() {
        return None;
    }
    let paused = state.profile_switching_paused();
    let description = if let Some(app) = state.locked_profile() {
        let profile = if app.is_empty() {
            tr!("common.default").to_string()
        } else {
            profile_name(state, app)
        };
        tr!("profiles.locked_profile_value", profile => profile)
    } else {
        tr!("profiles.pause_switching_description")
    };
    let last_automatic = state.last_autoswitched_profile();
    let last_profile = last_automatic.map_or_else(
        || tr!("profiles.no_automatic_switch"),
        |last| {
            let profile = last.profile().map_or_else(
                || tr!("common.default").to_string(),
                |key| state.profile_name(key),
            );
            tr!("profiles.last_autoswitched_profile", profile => profile)
        },
    );
    let source = last_automatic.map(|last| {
        let app = last.application();
        tr!("profiles.autoswitch_application", app => app.display_name.clone(), path => app.id.clone())
    });
    let pal = theme::palette(cx);
    Some(
        div()
            .flex_none()
            .w_full()
            .border_t_1()
            .border_color(pal.border)
            .px_4()
            .py_3()
            .child(
                div()
                    .id("profile-switching-help")
                    .tooltip(|window, cx| {
                        crate::ui::components::detailed_tooltip(
                            tr!("profiles.pause_switching_description"),
                            window,
                            cx,
                        )
                    })
                    .child(toggle_row(
                        "profile-switching-footer",
                        pal,
                        tr!("profiles.pause_switching"),
                        description,
                        Toggle::new("automatic-profile-switching")
                            .selected(!paused)
                            .on_change(|automatic, _, cx| {
                                AppState::apply(cx, |state| {
                                    state.set_profile_switching_paused(!*automatic)
                                });
                            }),
                    )),
            )
            .child(
                div()
                    .id("last-autoswitched-profile")
                    .debug_selector(|| "last-autoswitched-profile".into())
                    .mt_1()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .when_some(source, |row, source| {
                        row.tooltip(move |window, cx| {
                            crate::ui::components::detailed_tooltip(source.clone(), window, cx)
                        })
                    })
                    .child(last_profile),
            ),
    )
}

/// Profile inheritance and active-app context shown above the device canvas.
pub(crate) fn profile_canvas_status(cx: &App) -> Option<gpui::Div> {
    let pal = theme::palette(cx);
    let state = AppState::try_read(cx)?;
    if !state.current_device_is_persistent() {
        return None;
    }
    let editing_app = state.editing_app().map(|app| profile_name(state, app));
    let override_count = state.editing_app_overrides().map_or(0, BTreeMap::len);
    let summary = profile_summary(editing_app.as_deref(), override_count);
    let active = state
        .active_profile_name()
        .map_or_else(|| tr!("common.default"), gpui::SharedString::from);

    Some(
        h_flex()
            .flex_none()
            .w_full()
            .items_start()
            .gap_3()
            .px_4()
            .pt_4()
            .text_caption()
            .text_color(pal.text_muted)
            .child(div().flex_1().min_w_0().child(summary))
            .child(
                div()
                    .flex_none()
                    .child(tr!("profiles.active_profile_value", profile => active))
                    .children(
                        state
                            .profile_switching_paused()
                            .then(|| tr!("profiles.switching_paused")),
                    ),
            ),
    )
}

fn profile_summary(editing_app: Option<&str>, override_count: usize) -> gpui::SharedString {
    let Some(app) = editing_app else {
        return tr!("profiles.default_bindings_description");
    };
    match override_count {
        0 => tr!(
            "profiles.app_profile_no_overrides",
            app => app
        ),
        1 => tr!(
            "profiles.app_profile_single_override",
            app => app
        ),
        count => tr!(
            "profiles.app_profile_override_count",
            app => app,
            count => count
        ),
    }
}

#[derive(Clone, Copy)]
enum ProfileCommand {
    ResetButtons,
    RemoveButtons,
    ResetRing,
    RemoveRing,
    RemoveAll,
}

impl ProfileCommand {
    fn apply(self, key: &DeviceKey, app: &str, cx: &mut App) {
        AppState::apply(cx, |state| match self {
            Self::ResetButtons => state.reset_app_profile(key, app),
            Self::RemoveButtons => state.remove_app_profile(key, app),
            Self::ResetRing => state.reset_action_ring_profile(key, app),
            Self::RemoveRing => state.remove_action_ring_profile(key, app),
            Self::RemoveAll => state.remove_all_app_profiles(key, app),
        });
    }

    fn confirm(self, key: &DeviceKey, profile: &ProfileChoice, window: &mut Window, cx: &mut App) {
        // A draft in this editor may still have saved settings in the other.
        if !profile.persisted && !matches!(self, Self::RemoveAll) {
            self.apply(key, &profile.app, cx);
            return;
        }
        let key = key.clone();
        let app = profile.app.clone();
        let name = profile.name.clone();
        let reset = matches!(self, Self::ResetButtons | Self::ResetRing);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let key = key.clone();
            let app = app.clone();
            let question = if matches!(self, Self::RemoveAll) {
                tr!("profiles.remove_all_profiles_question", app => name.clone())
            } else if reset {
                tr!("profiles.reset_profile_question", app => name.clone())
            } else {
                tr!("profiles.remove_profile_question", app => name.clone())
            };
            let description = match self {
                Self::ResetButtons => tr!("profiles.reset_buttons_description"),
                Self::RemoveButtons => tr!("profiles.remove_buttons_description"),
                Self::ResetRing => tr!("profiles.reset_ring_description"),
                Self::RemoveRing => tr!("profiles.remove_ring_description"),
                Self::RemoveAll => tr!("profiles.remove_all_profiles_description"),
            };
            alert
                .title(question)
                .description(description)
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(if matches!(self, Self::RemoveAll) {
                            tr!("profiles.remove_all_profiles")
                        } else if reset {
                            tr!("profiles.reset_profile")
                        } else {
                            tr!("profiles.remove_profile")
                        })
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok(move |_event, _window, cx| {
                    self.apply(&key, &app, cx);
                    true
                })
        });
    }
}

#[cfg(test)]
mod tests {
    mod alias_profiles {
        use openlogi_core::app::ForegroundApp;
        use openlogi_core::config::Config;
        use openlogi_ipc::ForegroundApps;

        use super::super::{profile_name, scope_profiles};
        use crate::services::assets::AssetResolver;
        use crate::state::tests::{KNOWN_MOUSE_KEY, direct_inventory};
        use crate::state::{AppState, Sources};

        /// The known mouse with a Back override under each of `profiles`,
        /// `apps` as its `[apps]` table, and `recent` as the applications the
        /// agent saw in front. Loaded from a file, so a profile naming an
        /// alias `apps` lacks fails here exactly as it does at startup.
        ///
        /// Every alias below lists its identifier under all three platform
        /// fields, in lower case, so the fixture means the same on each host.
        fn state_with(apps: &str, profiles: &[&str], recent: &[(&str, &str)]) -> AppState {
            let mut source = String::from("schema_version = 9\n");
            source.extend(profiles.iter().map(|key| {
                format!(
                    "[devices.\"{KNOWN_MOUSE_KEY}\".per_app_bindings.\"{key}\"]\nBack = \"Undo\"\n"
                )
            }));
            source.push_str(apps);
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(openlogi_core::paths::CONFIG_FILE);
            std::fs::write(&path, source).expect("write config");
            let config = Config::load_from_path(&path).expect("fixture config loads");
            let resolver = AssetResolver::new();
            let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
            let mut state = AppState::new(Sources {
                inventories: &[direct_inventory([0xa3, 0x93, 0xca, 0xe0])],
                ..Sources::in_memory(config, &resolver, commands)
            });
            let recent: Vec<ForegroundApp> = recent
                .iter()
                .map(|(id, name)| ForegroundApp {
                    id: (*id).to_string(),
                    display_name: (*name).to_string(),
                })
                .collect();
            let _ = state.set_foreground(ForegroundApps {
                current: recent.first().cloned(),
                recent,
            });
            state
        }

        const EDITOR: &str = "[apps.editor]\nmacos = \"editor-id\"\nlinux = \"editor-id\"\nwindows = \"editor-id\"\n";
        const NAMED_EDITOR: &str = "[apps.editor]\nname = \"Editor\"\nmacos = \"editor-id\"\nlinux = \"editor-id\"\nwindows = \"editor-id\"\n";

        #[test]
        fn an_alias_profile_is_named_and_illustrated_as_its_application() {
            let named = state_with(
                NAMED_EDITOR,
                &["app:editor"],
                &[("editor-id", "Editor Pro")],
            );
            assert_eq!(
                profile_name(&named, "app:editor"),
                "Editor",
                "the alias's own name wins"
            );
            let (profiles, _) = scope_profiles(&named, &["app:editor"], None);
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].app, "app:editor");
            assert_eq!(
                profiles[0].icon, "editor-id",
                "the icon is the application's, not the alias key's"
            );

            let seen = state_with(EDITOR, &["app:editor"], &[("editor-id", "Editor Pro")]);
            assert_eq!(
                profile_name(&seen, "app:editor"),
                "Editor Pro",
                "an unnamed alias takes the name the agent reported for it here"
            );

            let unseen = state_with(EDITOR, &["app:editor"], &[]);
            assert_eq!(profile_name(&unseen, "app:editor"), "editor");
        }

        #[test]
        fn a_profile_takes_the_name_of_an_application_it_matches_by_executable() {
            // Windows reports the full path; the profile names only the
            // executable, directly or through an alias.
            const TOOL: &str = "[apps.tool]\nmacos = \"exe:tool.exe\"\nlinux = \"exe:tool.exe\"\nwindows = \"exe:tool.exe\"\n";
            let state = state_with(
                TOOL,
                &["app:tool", "exe:tool.exe"],
                &[(r"c:\tools\tool.exe", "Tool")],
            );
            assert_eq!(profile_name(&state, "app:tool"), "Tool");
            assert_eq!(profile_name(&state, "exe:tool.exe"), "Tool");
        }

        #[test]
        fn a_platform_profile_keeps_its_reported_or_derived_name() {
            let state = state_with(
                "",
                &["com.example.Native"],
                &[("com.example.Native", "Native")],
            );
            assert_eq!(profile_name(&state, "com.example.Native"), "Native");
            assert_eq!(profile_name(&state, "com.example.Unseen"), "Unseen");
            let (profiles, _) = scope_profiles(&state, &["com.example.Native"], None);
            assert_eq!(profiles[0].icon, "com.example.Native");
        }

        #[test]
        fn an_application_already_under_a_profile_is_not_offered_again() {
            let state = state_with(
                EDITOR,
                &["app:editor", "exe:tool.exe"],
                &[("editor-id", "Editor"), ("other-id", "Other")],
            );
            let (profiles, coverage) =
                scope_profiles(&state, &["app:editor", "exe:tool.exe"], Some("other-id"));
            assert!(
                coverage.covers("editor-id"),
                "the alias profile is the one Editor runs under"
            );
            assert!(
                coverage.covers(r"c:\tools\tool.exe"),
                "so is the executable profile for a path"
            );
            assert!(
                !coverage.covers("other-id"),
                "an unsaved draft covers nothing yet"
            );
            let draft = profiles
                .iter()
                .find(|profile| profile.app == "other-id")
                .expect("the edited application gets a draft tab");
            assert!(!draft.persisted);
            assert_eq!(draft.name, "Other");
        }
    }
}
