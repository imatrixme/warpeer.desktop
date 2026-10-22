use warpui::elements::{Empty, ParentElement};
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use super::OnboardingIntention;

#[derive(Clone, Debug)]
pub struct OnboardingKeybindings {
    pub toggle_input_mode: String,
    pub submit_to_local_agent: String,
    pub submit_to_cloud_agent: String,
    pub return_to_terminal_mode: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalState {
    Submit,
    Skip,
    Finish,
    Initialize,
    BackToTerminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnboardingQuery {
    TerminalCommand(String),
    AgentPrompt(String),
    None,
}

#[derive(Clone, Debug)]
pub enum OnboardingCalloutViewEvent {
    StateUpdated,
    Completed { final_state: FinalState },
    EnterAgentModality,
    NaturalLanguageDetectionToggled(bool),
}

#[derive(Clone, Debug)]
pub enum OnboardingCalloutViewAction {
    NextClicked,
    SkipClicked,
    BackToTerminalClicked,
    ToggleCheckbox,
}

pub struct OnboardingCalloutView {
    has_project: bool,
}

impl OnboardingCalloutView {
    pub fn new_universal_input(
        has_project: bool,
        initial_natural_language_detection_enabled: bool,
        keybindings: OnboardingKeybindings,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let _ = initial_natural_language_detection_enabled;
        drop(keybindings);
        let _ = ctx;
        Self { has_project }
    }

    pub fn new_agent_modality(
        has_project: bool,
        intention: OnboardingIntention,
        initial_natural_language_detection_enabled: bool,
        keybindings: OnboardingKeybindings,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let _ = intention;
        let _ = initial_natural_language_detection_enabled;
        drop(keybindings);
        let _ = ctx;
        Self { has_project }
    }

    pub fn has_project(&self, app: &AppContext) -> bool {
        let _ = app;
        self.has_project
    }

    pub fn start_onboarding(&mut self, ctx: &mut ViewContext<Self>) {
        let _ = ctx;
    }

    pub fn is_onboarding_active(&self, app: &AppContext) -> bool {
        let _ = app;
        false
    }

    pub fn prompt_string(&self, app: &AppContext) -> String {
        let _ = app;
        String::new()
    }

    pub fn prompt(&self, app: &AppContext) -> OnboardingQuery {
        let _ = app;
        OnboardingQuery::None
    }

    pub fn should_position_above_zero_state(&self, app: &AppContext) -> bool {
        let _ = app;
        false
    }
}

impl Entity for OnboardingCalloutView {
    type Event = OnboardingCalloutViewEvent;
}

impl View for OnboardingCalloutView {
    fn ui_name() -> &'static str {
        "OnboardingCalloutView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = app;
        Empty::new().finish()
    }
}

impl TypedActionView for OnboardingCalloutView {
    type Action = OnboardingCalloutViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        let _ = action;
        let _ = ctx;
    }
}
