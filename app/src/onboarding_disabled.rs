use ai::LLMId;
use serde_json::Value;
use warp_core::telemetry::{EnablementState, TelemetryEvent, TelemetryEventDesc};
use warp_core::ui::theme::WarpTheme;
use warpui::elements::{Empty, ParentElement};
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

#[path = "onboarding_disabled/callout.rs"]
pub mod callout;
#[path = "onboarding_disabled/components.rs"]
pub mod components;
#[path = "onboarding_disabled/slides.rs"]
pub mod slides;

pub use callout::{OnboardingCalloutView, OnboardingKeybindings};
pub use slides::{OfferVariant, ProjectOnboardingSettings};

pub const WARP_DRIVE_FEATURES: &[&str] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnboardingIntention {
    Terminal,
    AgentDrivenDevelopment,
}

impl std::fmt::Display for OnboardingIntention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal => f.write_str("terminal"),
            Self::AgentDrivenDevelopment => f.write_str("agent_driven"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionDefault {
    #[default]
    Agent,
    Terminal,
}

#[derive(Clone, Debug)]
pub struct UICustomizationSettings {
    pub use_vertical_tabs: bool,
    pub show_conversation_history: bool,
    pub show_project_explorer: bool,
    pub show_global_search: bool,
    pub show_warp_drive: bool,
    pub show_code_review_button: bool,
}

impl UICustomizationSettings {
    pub fn agent_defaults() -> Self {
        Self {
            use_vertical_tabs: true,
            show_conversation_history: false,
            show_project_explorer: true,
            show_global_search: true,
            show_warp_drive: false,
            show_code_review_button: true,
        }
    }

    pub fn terminal_defaults() -> Self {
        Self {
            use_vertical_tabs: false,
            show_conversation_history: false,
            show_project_explorer: true,
            show_global_search: true,
            show_warp_drive: false,
            show_code_review_button: true,
        }
    }

    pub fn tools_panel_enabled(&self, intention: &OnboardingIntention) -> bool {
        let show_conversation = matches!(intention, OnboardingIntention::AgentDrivenDevelopment)
            && self.show_conversation_history;
        show_conversation || self.show_project_explorer || self.show_global_search
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnboardingAuthState {
    LoggedOut,
    FreeUser,
    PayingUser,
}

#[derive(Clone, Debug)]
pub enum SelectedSettings {
    Terminal {
        ui_customization: Option<UICustomizationSettings>,
        cli_agent_toolbar_enabled: bool,
        show_agent_notifications: bool,
    },
    AgentDrivenDevelopment {
        agent_settings: slides::AgentDevelopmentSettings,
        project_settings: ProjectOnboardingSettings,
        ui_customization: Option<UICustomizationSettings>,
    },
}

impl SelectedSettings {
    pub fn is_ai_enabled(&self) -> bool {
        false
    }

    pub fn is_warp_drive_enabled(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreditPackOption {
    pub credits: i32,
    pub price_usd_cents: i32,
    pub savings_percent: u32,
}

impl CreditPackOption {
    pub fn price_label(&self) -> String {
        if self.price_usd_cents % 100 == 0 {
            format!("${}", self.price_usd_cents / 100)
        } else {
            format!("${:.2}", self.price_usd_cents as f64 / 100.)
        }
    }

    pub fn credits_label(&self) -> String {
        self.credits.to_string()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChooseHowToStartExperimentArm {
    #[default]
    Unassigned,
    Control,
    Experiment,
}

impl ChooseHowToStartExperimentArm {
    pub fn telemetry_value(self) -> &'static str {
        match self {
            Self::Unassigned => "unassigned",
            Self::Control => "control",
            Self::Experiment => "experiment",
        }
    }

    pub fn shows_credit_packs(self) -> bool {
        matches!(self, Self::Experiment)
    }
}

#[derive(Clone, Debug)]
pub enum OnboardingEvent {
    OnboardingStarted,
    SlideViewed {
        slide_name: String,
    },
    SettingChanged {
        setting: String,
        value: String,
    },
    OnboardingSlidesCompleted {
        intention: String,
        model: Option<String>,
        autonomy: Option<String>,
        has_project_path: bool,
        ai_access: Option<String>,
    },
    GetStartedClicked,
    FolderSelectionStarted,
    FolderSelected,
    CalloutDisplayed {
        callout: String,
    },
    CalloutNext,
    CalloutCompleted {
        completion_type: String,
    },
    SlideNavigatedNext,
    SlideNavigatedBack,
    NoAiConfirmationShown,
    NoAiConfirmed,
    NoAiConfirmationCancelled,
    AgentSlideUpgradeClicked,
    WelcomeLoginClicked,
    OnboardingAction {
        slide_name: String,
        action: String,
        account_class: Option<String>,
    },
    OnboardingAuthCompleted {
        account_class: String,
        has_team: bool,
        is_paid: bool,
        team_discovery_outcome: String,
    },
    OnboardingUpgradeStarted {
        source_slide: String,
        account_class: String,
    },
    OnboardingUpgradeCompleted {
        source_slide: String,
        account_class: String,
    },
    OnboardingCompleted {
        completion_type: String,
    },
}

warp_core::register_telemetry_event!(OnboardingEvent);

impl TelemetryEvent for OnboardingEvent {
    fn name(&self) -> &'static str {
        "onboarding_disabled"
    }

    fn payload(&self) -> Option<Value> {
        None
    }

    fn description(&self) -> &'static str {
        "Disabled onboarding event"
    }

    fn enablement_state(&self) -> EnablementState {
        EnablementState::Always
    }

    fn contains_ugc(&self) -> bool {
        false
    }

    fn event_descs() -> impl Iterator<Item = Box<dyn TelemetryEventDesc>> {
        std::iter::empty()
    }
}

#[derive(Clone, Debug)]
pub enum AgentOnboardingEvent {
    ThemeSelected { theme_name: String },
    SyncWithOsToggled { enabled: bool },
    OnboardingCompleted(SelectedSettings),
    OnboardingSkipped,
    LoginFromWelcomeRequested,
    PrivacySettingsFromTerminalThemeSlideRequested,
    UpgradeRequested,
    UpgradeCopyUrlRequested,
    UpgradePasteTokenFromClipboardRequested,
    OfferSetUpLaterSelected { variant: OfferVariant },
    OfferAiSellSatisfied { variant: OfferVariant },
    AppBecameActive,
}

#[derive(Clone, Copy, Debug)]
pub enum AgentOnboardingAction {
    UpKey,
    DownKey,
    LeftKey,
    RightKey,
    TabKey,
    EnterKey,
    CmdOrCtrlEnterKey,
    Escape,
    NoAiConfirm,
    NoAiCancel,
    NoAiDismiss,
    DismissPlanActivatedToast,
}

pub struct AgentOnboardingView {
    awaiting_credits: bool,
    experiment_arm: ChooseHowToStartExperimentArm,
}

impl AgentOnboardingView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        themes: [WarpTheme; 4],
        skippable: bool,
        models: Vec<slides::OnboardingModelInfo>,
        default_model_id: LLMId,
        workspace_enforces_autonomy: bool,
        auth_state: OnboardingAuthState,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        drop(themes);
        let _ = skippable;
        drop(models);
        drop(default_model_id);
        let _ = workspace_enforces_autonomy;
        let _ = auth_state;
        let _ = ctx;
        Self {
            awaiting_credits: false,
            experiment_arm: ChooseHowToStartExperimentArm::Unassigned,
        }
    }

    pub fn set_onboarding_models(
        &mut self,
        models: Vec<slides::OnboardingModelInfo>,
        default_model_id: LLMId,
        ctx: &mut ViewContext<Self>,
    ) {
        drop(models);
        drop(default_model_id);
        let _ = ctx;
    }

    pub fn set_pricing_promotion_message(
        &mut self,
        message: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        drop(message);
        let _ = ctx;
    }

    pub fn set_workspace_enforces_autonomy(&mut self, value: bool, ctx: &mut ViewContext<Self>) {
        let _ = value;
        let _ = ctx;
    }

    pub fn set_auth_state(&mut self, auth_state: OnboardingAuthState, ctx: &mut ViewContext<Self>) {
        let _ = auth_state;
        let _ = ctx;
    }

    pub fn set_credit_pack_options(
        &mut self,
        options: Vec<CreditPackOption>,
        ctx: &mut ViewContext<Self>,
    ) {
        drop(options);
        let _ = ctx;
    }

    pub fn on_credit_purchase_checkout_opened(&mut self, ctx: &mut ViewContext<Self>) {
        self.awaiting_credits = true;
        let _ = ctx;
    }

    pub fn on_ai_credit_availability_observed(
        &mut self,
        available: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if available {
            self.awaiting_credits = false;
        }
        let _ = ctx;
    }

    pub fn on_checkout_succeeded(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let _ = ctx;
        false
    }

    pub fn on_credit_purchase_completed(&mut self, ctx: &mut ViewContext<Self>) {
        self.awaiting_credits = false;
        let _ = ctx;
    }

    pub fn on_credit_purchase_failed(&mut self, ctx: &mut ViewContext<Self>) {
        self.awaiting_credits = false;
        let _ = ctx;
    }

    pub fn is_awaiting_purchased_credits(&self, ctx: &AppContext) -> bool {
        let _ = ctx;
        self.awaiting_credits
    }

    pub fn set_choose_how_to_start_experiment_arm(
        &mut self,
        arm: ChooseHowToStartExperimentArm,
        ctx: &mut ViewContext<Self>,
    ) {
        self.experiment_arm = arm;
        let _ = ctx;
    }

    pub fn offer_experiment_arm(&self, ctx: &AppContext) -> Option<String> {
        let _ = ctx;
        Some(self.experiment_arm.telemetry_value().to_string())
    }

    pub fn show_post_auth_offer(&mut self, variant: OfferVariant, ctx: &mut ViewContext<Self>) {
        let _ = variant;
        let _ = ctx;
    }

    pub fn use_vertical_tabs(&self, ctx: &AppContext) -> bool {
        let _ = ctx;
        true
    }

    pub fn start_onboarding(&self, ctx: &mut ViewContext<Self>) {
        let _ = ctx;
    }
}

impl Entity for AgentOnboardingView {
    type Event = AgentOnboardingEvent;
}

impl View for AgentOnboardingView {
    fn ui_name() -> &'static str {
        "AgentOnboardingView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = app;
        Empty::new().finish()
    }
}

impl TypedActionView for AgentOnboardingView {
    type Action = AgentOnboardingAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        let _ = action;
        let _ = ctx;
    }
}

pub fn init(app: &mut AppContext) {
    let _ = app;
}
