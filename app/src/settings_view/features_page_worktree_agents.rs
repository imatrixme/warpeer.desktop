use ::settings::{Setting as _, ToggleableSetting as _};
use warp_errors::report_if_error;
use warpui::elements::Element;
use warpui::keymap::ContextPredicate;
use warpui::ui_components::components::UiComponent;
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{Action, AppContext, SingletonEntity as _, ViewContext, ViewHandle};

use super::{FeaturesPageAction, FeaturesPageView};
use crate::appearance::Appearance;
use crate::settings_view::settings_page::{
    LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item, render_dropdown_item,
};
use crate::settings_view::{SettingsAction, ToggleSettingActionPair, flags};
use crate::view_components::{Dropdown, DropdownItem};
use crate::worktree_agents::{
    WorktreeAgentPaneLinkEffect, WorktreeAgentSettings, WorktreeAgentSortKey,
};

pub(super) fn widgets() -> Vec<Box<dyn SettingsWidget<View = FeaturesPageView>>> {
    vec![
        Box::new(ShowInPaneHeaderWidget::default()),
        Box::new(PaneLinkEffectWidget),
        Box::new(ReuseCurrentPaneWidget::default()),
        Box::new(DefaultSortWidget),
        Box::new(DefaultSortDirectionWidget::default()),
    ]
}

pub(super) fn toggle_binding_pairs<T: Action + Clone>(
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) -> Vec<ToggleSettingActionPair<T>> {
    vec![
        ToggleSettingActionPair::new(
            "worktree agent control center",
            builder(SettingsAction::FeaturesPageToggle(
                FeaturesPageAction::ToggleWorktreeAgentHeader,
            )),
            context,
            flags::WORKTREE_AGENT_HEADER_CONTEXT_FLAG,
        ),
        ToggleSettingActionPair::new(
            "current pane reuse for worktree agents",
            builder(SettingsAction::FeaturesPageToggle(
                FeaturesPageAction::ToggleWorktreeAgentCurrentPaneReuse,
            )),
            context,
            flags::WORKTREE_AGENT_REUSE_CURRENT_PANE_CONTEXT_FLAG,
        ),
        ToggleSettingActionPair::new(
            "descending worktree agent sorting",
            builder(SettingsAction::FeaturesPageToggle(
                FeaturesPageAction::ToggleWorktreeAgentSortDescending,
            )),
            context,
            flags::WORKTREE_AGENT_SORT_DESCENDING_CONTEXT_FLAG,
        ),
    ]
}

pub(super) fn toggle_show_in_pane_header(ctx: &mut ViewContext<FeaturesPageView>) {
    WorktreeAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.show_in_pane_header.toggle_and_save_value(ctx));
    });
}

pub(super) fn toggle_reuse_current_pane(ctx: &mut ViewContext<FeaturesPageView>) {
    WorktreeAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.reuse_current_pane.toggle_and_save_value(ctx));
    });
}

pub(super) fn set_default_sort(
    sort: WorktreeAgentSortKey,
    ctx: &mut ViewContext<FeaturesPageView>,
) {
    WorktreeAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.default_sort.set_value(sort, ctx));
    });
}

pub(super) fn set_pane_link_effect(
    effect: WorktreeAgentPaneLinkEffect,
    ctx: &mut ViewContext<FeaturesPageView>,
) {
    WorktreeAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.pane_link_effect.set_value(effect, ctx));
    });
}

pub(super) fn toggle_default_sort_descending(ctx: &mut ViewContext<FeaturesPageView>) {
    WorktreeAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.default_sort_descending.toggle_and_save_value(ctx));
    });
}

pub(super) fn update_sort_dropdown(
    dropdown: ViewHandle<Dropdown<FeaturesPageAction>>,
    ctx: &mut ViewContext<FeaturesPageView>,
) {
    dropdown.update(ctx, |dropdown, ctx| {
        let values = [
            WorktreeAgentSortKey::Auto,
            WorktreeAgentSortKey::Status,
            WorktreeAgentSortKey::LastActive,
            WorktreeAgentSortKey::TokenUsage,
            WorktreeAgentSortKey::ContextRemaining,
            WorktreeAgentSortKey::SessionName,
        ];
        let current = *WorktreeAgentSettings::as_ref(ctx).default_sort;
        dropdown.set_items(
            values
                .into_iter()
                .map(|sort| {
                    DropdownItem::new(sort.label(), FeaturesPageAction::SetWorktreeAgentSort(sort))
                })
                .collect(),
            ctx,
        );
        dropdown.set_selected_by_action(FeaturesPageAction::SetWorktreeAgentSort(current), ctx);
    });
}

pub(super) fn update_pane_link_dropdown(
    dropdown: ViewHandle<Dropdown<FeaturesPageAction>>,
    ctx: &mut ViewContext<FeaturesPageView>,
) {
    dropdown.update(ctx, |dropdown, ctx| {
        let values = [
            WorktreeAgentPaneLinkEffect::Off,
            WorktreeAgentPaneLinkEffect::Highlight,
            WorktreeAgentPaneLinkEffect::Pulse,
        ];
        let current = *WorktreeAgentSettings::as_ref(ctx).pane_link_effect;
        dropdown.set_items(
            values
                .into_iter()
                .map(|effect| {
                    DropdownItem::new(
                        effect.label(),
                        FeaturesPageAction::SetWorktreeAgentPaneLinkEffect(effect),
                    )
                })
                .collect(),
            ctx,
        );
        dropdown.set_selected_by_action(
            FeaturesPageAction::SetWorktreeAgentPaneLinkEffect(current),
            ctx,
        );
    });
}

#[derive(Default)]
struct ShowInPaneHeaderWidget {
    switch: SwitchStateHandle,
}

impl SettingsWidget for ShowInPaneHeaderWidget {
    type View = FeaturesPageView;

    fn search_terms(&self) -> &str {
        "worktree agent control center monitoring discovery pane header title bar group avatars visibility tmux herdr"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<FeaturesPageAction>(
            "Worktree agent control center".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            appearance
                .ui_builder()
                .switch(self.switch.clone())
                .check(*WorktreeAgentSettings::as_ref(app).show_in_pane_header)
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(FeaturesPageAction::ToggleWorktreeAgentHeader)
                })
                .finish(),
            Some(
                "Show worktree and agent controls in terminal pane headers. Turning this off stops local agent discovery and releases collected session data."
                    .into(),
            ),
        )
    }
}

struct PaneLinkEffectWidget;

impl SettingsWidget for PaneLinkEffectWidget {
    type View = FeaturesPageView;

    fn search_terms(&self) -> &str {
        "worktree agent attached pane relationship highlight pulse border hover"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_dropdown_item(
            appearance,
            "Attached pane indicator:",
            Some("Shown while hovering an attached agent"),
            None,
            LocalOnlyIconState::Hidden,
            None,
            &view.worktree_agent_pane_link_dropdown,
        )
    }
}

#[derive(Default)]
struct ReuseCurrentPaneWidget {
    switch: SwitchStateHandle,
}

struct DefaultSortWidget;

impl SettingsWidget for DefaultSortWidget {
    type View = FeaturesPageView;

    fn search_terms(&self) -> &str {
        "worktree agent default sort auto status activity tokens context session name order"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_dropdown_item(
            appearance,
            "Default agent sorting:",
            Some("Applied when the hover panel opens"),
            None,
            LocalOnlyIconState::Hidden,
            None,
            &view.worktree_agent_sort_dropdown,
        )
    }
}

#[derive(Default)]
struct DefaultSortDirectionWidget {
    switch: SwitchStateHandle,
}

impl SettingsWidget for DefaultSortDirectionWidget {
    type View = FeaturesPageView;

    fn search_terms(&self) -> &str {
        "worktree agent default sort direction ascending descending order"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<FeaturesPageAction>(
            "Descending sort order".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            appearance
                .ui_builder()
                .switch(self.switch.clone())
                .check(*WorktreeAgentSettings::as_ref(app).default_sort_descending)
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(FeaturesPageAction::ToggleWorktreeAgentSortDescending)
                })
                .finish(),
            Some("Ignored while the default sorting mode is Auto.".into()),
        )
    }
}

impl SettingsWidget for ReuseCurrentPaneWidget {
    type View = FeaturesPageView;

    fn search_terms(&self) -> &str {
        "worktree agent click action activate attach resume current pane new pane behavior"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<FeaturesPageAction>(
            "Reuse current pane when activating".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            appearance
                .ui_builder()
                .switch(self.switch.clone())
                .check(*WorktreeAgentSettings::as_ref(app).reuse_current_pane)
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(
                        FeaturesPageAction::ToggleWorktreeAgentCurrentPaneReuse,
                    )
                })
                .finish(),
            Some(
                "When no exact pane is available, attach or resume in the current pane instead of opening a new one."
                    .into(),
            ),
        )
    }
}
