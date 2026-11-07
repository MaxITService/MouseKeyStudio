//! Real inspector scenes, independent of keyboard images and hardware.
use super::*;
use crate::ui::snapshots::Scene;
use gpui::{AnyView, size};

struct Inspector {
    view: Entity<FunctionRowView>,
}

impl Render for Inspector {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = self.view.update(cx, |view, cx| {
            view.config_panel(
                0,
                &[KeySlot {
                    idx: 0,
                    label: "Calculator".into(),
                    target: KeyTarget::Control(ButtonId::control(0x000a)),
                    x_frac: 0.5,
                    y_frac: 0.1,
                    binding: "Workflow".into(),
                    binding_icon: None,
                    lcd_icon: None,
                }],
                &self.view,
                cx,
            )
        });
        gpui_component::v_flex().p_4().child(panel)
    }
}

fn build(window: &mut Window, cx: &mut App, empty_workflow: bool) -> AnyView {
    let view = cx.new(FunctionRowView::new);
    if empty_workflow {
        view.update(cx, |view, cx| {
            view.open_editor(PowerUserKind::Workflow, cx);
            let saved = Action::Workflow(vec![WorkflowStep::Delay { millis: 50 }]);
            view.prepare_editor(PowerUserKind::Workflow, Some(&saved), window, cx);
            view.remove_workflow_step(0, cx);
            view.prepare_editor(PowerUserKind::Workflow, Some(&saved), window, cx);
        });
    }
    cx.new(|_| Inspector { view }).into()
}

pub(crate) fn scenes() -> Vec<Scene> {
    vec![
        Scene {
            name: "keyboard/action-picker",
            size: size(px(420.), px(650.)),
            build: |window, cx| build(window, cx, false),
        },
        Scene {
            name: "keyboard/workflow-empty",
            size: size(px(420.), px(650.)),
            build: |window, cx| build(window, cx, true),
        },
    ]
}
