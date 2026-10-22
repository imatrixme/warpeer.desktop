use ai::LLMId;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::icons::Icon;
use warpui::Element;
use warpui::elements::{ClippedScrollStateHandle, Empty, ParentElement};

use super::SessionDefault;

#[derive(Clone, Debug)]
pub struct OnboardingModelInfo {
    pub id: LLMId,
    pub title: String,
    pub icon: Icon,
    pub is_default: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentAutonomy {
    Full,
    #[default]
    Partial,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDevelopmentSettings {
    pub selected_model_id: LLMId,
    pub autonomy: Option<AgentAutonomy>,
    pub cli_agent_toolbar_enabled: bool,
    pub session_default: SessionDefault,
    pub disable_oz: bool,
    pub show_agent_notifications: bool,
}

impl AgentDevelopmentSettings {
    pub fn new(default_model_id: LLMId) -> Self {
        Self {
            selected_model_id: default_model_id,
            autonomy: Some(AgentAutonomy::default()),
            cli_agent_toolbar_enabled: false,
            session_default: SessionDefault::Terminal,
            disable_oz: true,
            show_agent_notifications: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProjectOnboardingSettings {
    #[default]
    NoProject,
    Project {
        selected_local_folder: String,
        initialize_projects_automatically: bool,
    },
}

impl ProjectOnboardingSettings {
    pub fn from_path(path: Option<String>) -> Self {
        match path {
            Some(selected_local_folder) => Self::Project {
                selected_local_folder,
                initialize_projects_automatically: false,
            },
            None => Self::NoProject,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferVariant {
    HeadStart,
    ChooseHowToStart,
}

pub mod layout {
    use warpui::Element;
    use warpui::elements::{Empty, ParentElement};

    pub const ONBOARDING_BG_PATH: &str = "";
    pub const TWO_COLUMN_MIN_WIDTH: f32 = 0.;

    #[derive(Clone, Copy)]
    pub enum HPadding {
        Fixed(f32),
        ProportionalBoth(f32),
        ProportionalLeft(f32),
        ProportionalRight { ratio: f32, left_offset: f32 },
    }

    #[derive(Clone, Copy)]
    pub enum TopMode {
        Ratio(f32),
        CenterFraction {
            h_ratio: f32,
            inv_aspect: f32,
            frac: f32,
        },
    }

    #[derive(Clone, Copy)]
    pub enum ForegroundFit {
        CoverTopAligned,
        ContainTopAligned,
        ContainRightAligned,
    }

    #[derive(Clone, Copy)]
    pub struct ForegroundLayout {
        pub top_mode: TopMode,
        pub h_padding: HPadding,
        pub fit: ForegroundFit,
    }

    pub const FOREGROUND_LAYOUT_DEFAULT: ForegroundLayout = ForegroundLayout {
        top_mode: TopMode::Ratio(0.),
        h_padding: HPadding::Fixed(0.),
        fit: ForegroundFit::ContainTopAligned,
    };
    pub const FOREGROUND_LAYOUT_WIDE: ForegroundLayout = FOREGROUND_LAYOUT_DEFAULT;
    pub const FOREGROUND_LAYOUT_CODE_REVIEW: ForegroundLayout = FOREGROUND_LAYOUT_DEFAULT;
    pub const FOREGROUND_LAYOUT_THIRD_PARTY: ForegroundLayout = FOREGROUND_LAYOUT_DEFAULT;

    pub fn static_left(
        left: impl Fn() -> Box<dyn Element>,
        right: impl FnOnce() -> Box<dyn Element>,
    ) -> Box<dyn Element> {
        drop(right);
        left()
    }

    pub fn onboarding_right_panel_with_bg(
        path: &'static str,
        layout: ForegroundLayout,
    ) -> Box<dyn Element> {
        let _ = path;
        let _ = layout;
        Empty::new().finish()
    }
}

pub fn onboarding_bottom_nav(
    appearance: &Appearance,
    step_index: usize,
    step_count: usize,
    back_button: Option<Box<dyn Element>>,
    next_button: Option<Box<dyn Element>>,
) -> Box<dyn Element> {
    let _ = appearance;
    let _ = step_index;
    let _ = step_count;
    next_button
        .or(back_button)
        .unwrap_or_else(|| Empty::new().finish())
}

pub mod slide_content {
    use super::*;

    pub fn onboarding_slide_content(
        mut children: Vec<Box<dyn Element>>,
        bottom_nav: Box<dyn Element>,
        scroll_state: ClippedScrollStateHandle,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let _ = scroll_state;
        let _ = appearance;
        if children.is_empty() {
            bottom_nav
        } else {
            children.remove(0)
        }
    }
}
