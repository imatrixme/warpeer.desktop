use crate::product_profile::{self, ProductProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServiceComposition {
    FullWarp,
    LocalOnly,
}

impl ServiceComposition {
    pub(crate) fn active() -> Self {
        if cfg!(feature = "warpeer") || product_profile::active() == ProductProfile::TerminalOnly {
            Self::LocalOnly
        } else {
            Self::FullWarp
        }
    }

    pub(crate) fn allows_hosted_services(self) -> bool {
        self == Self::FullWarp
    }

    pub(crate) fn allows_product_education(self) -> bool {
        self == Self::FullWarp
    }
}

#[cfg(test)]
#[path = "service_composition_tests.rs"]
mod tests;
