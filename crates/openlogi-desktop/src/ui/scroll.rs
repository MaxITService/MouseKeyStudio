//! Shared scroll presentation. GPUI owns input smoothing for every component,
//! including third-party menus, virtual lists, dialogs and text editors.

use gpui::{
    App, Div, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    ScrollHandle, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
};
use gpui_component::scroll::{Scrollbar, ScrollbarMode};
use std::time::Duration;

/// Install the desktop's motion policy once, before opening any windows.
pub(crate) fn init(cx: &mut App) {
    cx.set_scroll_wheel_smoothing(Some(Duration::from_millis(120)));
}

/// A vertical panel with a native draggable scrollbar and smooth wheel input.
/// Its stable ID retains position while the region is mounted.
#[derive(IntoElement)]
pub(crate) struct SmoothScroll {
    id: ElementId,
    element: Div,
    handle: Option<ScrollHandle>,
    scrollbar_mode: ScrollbarMode,
}

impl SmoothScroll {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            element: div().relative(),
            handle: None,
            scrollbar_mode: ScrollbarMode::default(),
        }
    }

    /// Share an existing handle when the owner also scrolls programmatically.
    pub(crate) fn with_handle(mut self, handle: &ScrollHandle) -> Self {
        self.handle = Some(handle.clone());
        self
    }

    pub(crate) fn scrollbar_mode(mut self, mode: ScrollbarMode) -> Self {
        self.scrollbar_mode = mode;
        self
    }
}

impl gpui::InteractiveElement for SmoothScroll {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.element.interactivity()
    }
}

impl Styled for SmoothScroll {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl ParentElement for SmoothScroll {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.element.extend(elements);
    }
}

impl RenderOnce for SmoothScroll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "scroll"), cx, |_, _| {
            self.handle.unwrap_or_default()
        });
        let handle = state.read(cx).clone();
        self.element
            .id(self.id)
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .track_scroll(&handle)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(Scrollbar::vertical(&handle).mode(self.scrollbar_mode)),
            )
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "ui-snapshots")]
pub(crate) mod snapshots;
