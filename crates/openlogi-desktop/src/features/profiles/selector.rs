//! Retained searchable profile selection, keyed by profile identity.
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_component::{
    searchable_list::SearchableVec,
    select::{SelectEvent, SelectItem, SelectState},
};

use super::{ProfileScopeActions, ProfileScopeModel};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ProfileOption {
    app: Option<String>,
    name: String,
}

impl SelectItem for ProfileOption {
    type Value = Option<String>;
    fn title(&self) -> SharedString {
        self.name.clone().into()
    }
    fn value(&self) -> &Self::Value {
        &self.app
    }
}

type Options = SearchableVec<ProfileOption>;

pub(super) struct ProfileSelector {
    pub(super) select: Entity<SelectState<Options>>,
    shown: Vec<ProfileOption>,
    actions: ProfileScopeActions,
    _subscription: Subscription,
}

impl ProfileSelector {
    pub(super) fn new(
        actions: ProfileScopeActions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let select = cx.new(|cx| {
            SelectState::new(Options::new(Vec::new()), None, window, cx).searchable(true)
        });
        let subscription = cx.subscribe(&select, |this, _, event: &SelectEvent<Options>, cx| {
            if let SelectEvent::Confirm(Some(app)) = event {
                this.actions.select(app.clone(), cx);
            }
        });
        Self {
            select,
            shown: Vec::new(),
            actions,
            _subscription: subscription,
        }
    }

    pub(super) fn show(
        &mut self,
        model: &ProfileScopeModel,
        actions: ProfileScopeActions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.actions = actions;
        let options: Vec<_> = std::iter::once(ProfileOption {
            app: None,
            name: tr!("common.default").to_string(),
        })
        .chain(model.profiles.iter().map(|profile| ProfileOption {
            app: Some(profile.app.clone()),
            name: profile.name.clone(),
        }))
        .collect();
        let changed = self.shown != options;
        if changed {
            self.shown.clone_from(&options);
            self.select.update(cx, |select, cx| {
                select.set_items(Options::new(options), window, cx);
            });
        }
        if changed || self.select.read(cx).selected_value() != Some(&model.editing_app) {
            self.select.update(cx, |select, cx| {
                select.set_selected_value(&model.editing_app, window, cx);
            });
        }
    }
}
