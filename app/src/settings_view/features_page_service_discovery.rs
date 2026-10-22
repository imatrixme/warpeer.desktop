use ::settings::ToggleableSetting as _;
use warp_errors::report_if_error;
use warpui::elements::Element;
use warpui::keymap::ContextPredicate;
use warpui::ui_components::components::UiComponent;
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{Action, AppContext, SingletonEntity as _, ViewContext};

use super::{FeaturesPageAction, FeaturesPageView};
use crate::appearance::Appearance;
use crate::service_discovery_settings::{ServiceDiscoverySettings, is_service_discovery_available};
use crate::settings_view::settings_page::{
    LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item,
};
use crate::settings_view::{SettingsAction, ToggleSettingActionPair, flags};

#[derive(Clone, Copy)]
enum ServiceDiscoveryToggle {
    Enabled,
    OpenOnHover,
    CacheUsage,
    RuntimeDetails,
    ServerControls,
    DockerControls,
}

impl ServiceDiscoveryToggle {
    fn title(self) -> &'static str {
        match self {
            Self::Enabled => "Project service discovery",
            Self::OpenOnHover => "Open panel on hover",
            Self::CacheUsage => "Cache usage monitoring",
            Self::RuntimeDetails => "Process and port monitoring",
            Self::ServerControls => "Static and dynamic service controls",
            Self::DockerControls => "Docker and Compose controls",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Enabled => {
                "Show the project control panel in local terminal pane headers. Turning this off stops collection, clears snapshots, and stops services started by the panel."
            }
            Self::OpenOnHover => {
                "Open the panel by hovering the header icon; clicking always works."
            }
            Self::CacheUsage => {
                "Measure common dependency, build, and framework cache directories."
            }
            Self::RuntimeDetails => {
                "Collect related runtime processes, CPU, memory, and listening ports."
            }
            Self::ServerControls => {
                "Allow the panel to start static servers and package.json scripts."
            }
            Self::DockerControls => {
                "Discover Docker Compose services and expose start and stop controls."
            }
        }
    }

    fn search_terms(self) -> &'static str {
        match self {
            Self::Enabled => {
                "project service discovery control panel terminal pane header disable release memory"
            }
            Self::OpenOnHover => "project service discovery hover click panel",
            Self::CacheUsage => "cache disk usage node_modules rust target build dist next turbo",
            Self::RuntimeDetails => "process port pid cpu ram node bun workerd runtime monitor",
            Self::ServerControls => "static dynamic server package json scripts start stop",
            Self::DockerControls => "docker compose service group start stop container",
        }
    }

    fn action(self) -> FeaturesPageAction {
        match self {
            Self::Enabled => FeaturesPageAction::ToggleServiceDiscovery,
            Self::OpenOnHover => FeaturesPageAction::ToggleServiceDiscoveryHover,
            Self::CacheUsage => FeaturesPageAction::ToggleServiceDiscoveryCacheUsage,
            Self::RuntimeDetails => FeaturesPageAction::ToggleServiceDiscoveryRuntimeDetails,
            Self::ServerControls => FeaturesPageAction::ToggleServiceDiscoveryServerControls,
            Self::DockerControls => FeaturesPageAction::ToggleServiceDiscoveryDockerControls,
        }
    }

    fn value(self, app: &AppContext) -> bool {
        let settings = ServiceDiscoverySettings::as_ref(app);
        match self {
            Self::Enabled => *settings.enabled,
            Self::OpenOnHover => *settings.open_on_hover,
            Self::CacheUsage => *settings.show_cache_usage,
            Self::RuntimeDetails => *settings.show_runtime_details,
            Self::ServerControls => *settings.enable_server_controls,
            Self::DockerControls => *settings.enable_docker_controls,
        }
    }

    fn context_flag(self) -> &'static str {
        match self {
            Self::Enabled => flags::SERVICE_DISCOVERY_CONTEXT_FLAG,
            Self::OpenOnHover => flags::SERVICE_DISCOVERY_HOVER_CONTEXT_FLAG,
            Self::CacheUsage => flags::SERVICE_DISCOVERY_CACHE_CONTEXT_FLAG,
            Self::RuntimeDetails => flags::SERVICE_DISCOVERY_RUNTIME_CONTEXT_FLAG,
            Self::ServerControls => flags::SERVICE_DISCOVERY_SERVER_CONTEXT_FLAG,
            Self::DockerControls => flags::SERVICE_DISCOVERY_DOCKER_CONTEXT_FLAG,
        }
    }
}

const TOGGLES: [ServiceDiscoveryToggle; 6] = [
    ServiceDiscoveryToggle::Enabled,
    ServiceDiscoveryToggle::OpenOnHover,
    ServiceDiscoveryToggle::CacheUsage,
    ServiceDiscoveryToggle::RuntimeDetails,
    ServiceDiscoveryToggle::ServerControls,
    ServiceDiscoveryToggle::DockerControls,
];

pub(super) fn widgets() -> Vec<Box<dyn SettingsWidget<View = FeaturesPageView>>> {
    if !is_service_discovery_available() {
        return Vec::new();
    }
    TOGGLES
        .into_iter()
        .map(|kind| Box::new(ServiceDiscoveryWidget::new(kind)) as Box<_>)
        .collect()
}

pub(super) fn toggle_binding_pairs<T: Action + Clone>(
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) -> Vec<ToggleSettingActionPair<T>> {
    if !is_service_discovery_available() {
        return Vec::new();
    }
    TOGGLES
        .into_iter()
        .map(|kind| {
            ToggleSettingActionPair::new(
                kind.title(),
                builder(SettingsAction::FeaturesPageToggle(kind.action())),
                context,
                kind.context_flag(),
            )
        })
        .collect()
}

pub(super) fn toggle(kind: FeaturesPageAction, ctx: &mut ViewContext<FeaturesPageView>) {
    if !is_service_discovery_available() {
        return;
    }
    ServiceDiscoverySettings::handle(ctx).update(ctx, |settings, ctx| match kind {
        FeaturesPageAction::ToggleServiceDiscovery => {
            report_if_error!(settings.enabled.toggle_and_save_value(ctx));
        }
        FeaturesPageAction::ToggleServiceDiscoveryHover => {
            report_if_error!(settings.open_on_hover.toggle_and_save_value(ctx));
        }
        FeaturesPageAction::ToggleServiceDiscoveryCacheUsage => {
            report_if_error!(settings.show_cache_usage.toggle_and_save_value(ctx));
        }
        FeaturesPageAction::ToggleServiceDiscoveryRuntimeDetails => {
            report_if_error!(settings.show_runtime_details.toggle_and_save_value(ctx));
        }
        FeaturesPageAction::ToggleServiceDiscoveryServerControls => {
            report_if_error!(settings.enable_server_controls.toggle_and_save_value(ctx));
        }
        FeaturesPageAction::ToggleServiceDiscoveryDockerControls => {
            report_if_error!(settings.enable_docker_controls.toggle_and_save_value(ctx));
        }
        _ => {}
    });
}

struct ServiceDiscoveryWidget {
    kind: ServiceDiscoveryToggle,
    switch: SwitchStateHandle,
}

impl ServiceDiscoveryWidget {
    fn new(kind: ServiceDiscoveryToggle) -> Self {
        Self {
            kind,
            switch: Default::default(),
        }
    }
}

impl SettingsWidget for ServiceDiscoveryWidget {
    type View = FeaturesPageView;

    fn search_terms(&self) -> &str {
        self.kind.search_terms()
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let master_enabled = *ServiceDiscoverySettings::as_ref(app).enabled;
        render_body_item::<FeaturesPageAction>(
            self.kind.title().into(),
            None,
            LocalOnlyIconState::Hidden,
            if matches!(self.kind, ServiceDiscoveryToggle::Enabled) || master_enabled {
                ToggleState::Enabled
            } else {
                ToggleState::Disabled
            },
            appearance,
            appearance
                .ui_builder()
                .switch(self.switch.clone())
                .check(self.kind.value(app))
                .build()
                .on_click({
                    let action = self.kind.action();
                    move |ctx, _, _| ctx.dispatch_typed_action(action.clone())
                })
                .finish(),
            Some(self.kind.description().into()),
        )
    }
}
