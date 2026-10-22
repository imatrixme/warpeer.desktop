use std::fmt::{self, Display};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

uuid_id!(WorkerId);
uuid_id!(RequestId);
uuid_id!(ContextId);
uuid_id!(SurfaceId);
uuid_id!(ContainerId);
uuid_id!(PageId);
uuid_id!(SubscriptionId);
uuid_id!(ResourceHandleId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PluginId(String);

impl PluginId {
    pub fn parse(value: impl Into<String>) -> Result<Self, PluginIdError> {
        let value = value.into();
        let segments = value.split('.').collect::<Vec<_>>();
        let valid = segments.len() >= 3
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment.len() <= 63
                    && segment
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '-')
                    && segment
                        .chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_alphanumeric())
                    && segment
                        .chars()
                        .last()
                        .is_some_and(|character| character.is_ascii_alphanumeric())
            });
        if !valid || value.len() > 255 {
            return Err(PluginIdError(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for PluginId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("plugin id must be a reverse-DNS identifier: {0}")]
pub struct PluginIdError(String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Revision(pub u64);

impl Revision {
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
