use warpui::elements::Element;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::slider::SliderStateHandle;
use warpui::{AppContext, SingletonEntity as _};

use super::settings_page::{LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item};
use super::terminal_visuals_controls::TerminalVisualAction;
use super::terminal_visuals_page::{TerminalVisualsPageAction, TerminalVisualsSettingsPageView};
use crate::appearance::Appearance;
use crate::settings::{
    MAX_TERMINAL_PARTICLE_SPREAD, MIN_TERMINAL_PARTICLE_SPREAD, TerminalVisualSettings,
    normalized_terminal_particle_spread,
};

#[derive(Default)]
struct ParticleSpreadWidget {
    slider_state: SliderStateHandle,
}

impl SettingsWidget for ParticleSpreadWidget {
    type View = TerminalVisualsSettingsPageView;

    fn search_terms(&self) -> &str {
        "terminal erase particle spread range distance"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let value = normalized_terminal_particle_spread(
            *TerminalVisualSettings::as_ref(app).particle_spread,
        );
        render_body_item::<TerminalVisualsPageAction>(
            format!("Particle spread: {value:.1}x"),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            appearance
                .ui_builder()
                .slider(self.slider_state.clone())
                .with_range(MIN_TERMINAL_PARTICLE_SPREAD..MAX_TERMINAL_PARTICLE_SPREAD)
                .with_default_value(value)
                .with_style(UiComponentStyles {
                    width: Some(210.),
                    margin: Some(Coords::default().top(3.).bottom(3.)),
                    ..Default::default()
                })
                .on_drag(|ctx, _, value| {
                    let stepped = (value * 10.).round() / 10.;
                    ctx.dispatch_typed_action(TerminalVisualsPageAction::Apply(
                        TerminalVisualAction::SetParticleSpread(stepped),
                    ));
                })
                .on_change(|ctx, _, value| {
                    let stepped = (value * 10.).round() / 10.;
                    ctx.dispatch_typed_action(TerminalVisualsPageAction::Apply(
                        TerminalVisualAction::SetParticleSpread(stepped),
                    ));
                })
                .build()
                .finish(),
            Some(
                "Controls erase-particle range and density without changing particle lifetime."
                    .into(),
            ),
        )
    }
}

pub fn particle_spread_widget() -> Box<dyn SettingsWidget<View = TerminalVisualsSettingsPageView>> {
    Box::new(ParticleSpreadWidget::default())
}
