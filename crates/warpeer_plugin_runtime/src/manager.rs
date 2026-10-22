use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use warpeer_plugin_protocol::{Capability, PluginId, PluginManifest};

use crate::package::{ValidatedPackage, expanded_capabilities, is_downgrade, validate_package};
use crate::settings::{default_settings, normalize_settings, reconcile_settings};

#[derive(Clone, Debug)]
pub struct PluginPaths {
    pub root: PathBuf,
    pub packages: PathBuf,
    pub data: PathBuf,
    staging: PathBuf,
}

impl PluginPaths {
    pub fn new(product_data_root: impl Into<PathBuf>) -> Self {
        let root = product_data_root.into().join("plugins");
        Self {
            packages: root.join("packages"),
            data: root.join("data"),
            staging: root.join(".staging"),
            root,
        }
    }

    pub fn create(&self) -> Result<(), ManagerError> {
        for path in [&self.root, &self.packages, &self.data, &self.staging] {
            fs::create_dir_all(path).map_err(ManagerError::Io)?;
        }
        Ok(())
    }

    fn plugin_root(&self, id: &PluginId) -> PathBuf {
        self.packages.join(id.as_str())
    }

    fn version_root(&self, id: &PluginId, version: &Version) -> PathBuf {
        self.plugin_root(id)
            .join("versions")
            .join(version.to_string())
    }

    fn state_path(&self, id: &PluginId) -> PathBuf {
        self.plugin_root(id).join("state.json")
    }

    pub fn data_root(&self, id: &PluginId) -> PathBuf {
        self.data.join(id.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub trusted: bool,
    pub source: PluginSource,
    pub archive_hash: String,
    pub settings_revision: u64,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub grants: BTreeMap<Capability, BTreeSet<String>>,
}

impl InstalledPlugin {
    pub fn package_root(&self, paths: &PluginPaths) -> PathBuf {
        paths.version_root(&self.manifest.id, &self.manifest.version)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginSource {
    Bundled,
    Local,
}

#[derive(Debug)]
pub struct InstallReview {
    pub package: ValidatedPackage,
    pub previous: Option<InstalledPlugin>,
    pub expanded_capabilities: BTreeSet<Capability>,
    pub downgrade: bool,
    pub source: PluginSource,
    pub trusted: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InstallApproval {
    pub capability_expansion: bool,
    pub downgrade: bool,
}

pub struct PluginManager {
    paths: PluginPaths,
}

impl PluginManager {
    pub fn new(paths: PluginPaths) -> Result<Self, ManagerError> {
        paths.create()?;
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &PluginPaths {
        &self.paths
    }

    pub fn inspect_local(&self, package_path: &Path) -> Result<InstallReview, ManagerError> {
        let package = validate_package(package_path)?;
        self.review(package, PluginSource::Local, false)
    }

    pub fn inspect_bundled(
        &self,
        package: ValidatedPackage,
    ) -> Result<InstallReview, ManagerError> {
        self.review(package, PluginSource::Bundled, true)
    }

    pub fn install_bundled_if_newer(
        &self,
        package: ValidatedPackage,
    ) -> Result<Option<InstalledPlugin>, ManagerError> {
        if self
            .installed(&package.manifest.id)?
            .is_some_and(|installed| installed.manifest.version >= package.manifest.version)
        {
            return Ok(None);
        }
        let review = self.inspect_bundled(package)?;
        self.install(
            review,
            InstallApproval {
                capability_expansion: true,
                downgrade: false,
            },
        )
        .map(Some)
    }

    fn review(
        &self,
        package: ValidatedPackage,
        source: PluginSource,
        trusted: bool,
    ) -> Result<InstallReview, ManagerError> {
        let previous = self.installed(&package.manifest.id)?;
        let expanded_capabilities = previous
            .as_ref()
            .map(|current| expanded_capabilities(&current.manifest, &package.manifest))
            .unwrap_or_default();
        let downgrade = previous.as_ref().is_some_and(|current| {
            is_downgrade(&current.manifest.version, &package.manifest.version)
        });
        Ok(InstallReview {
            package,
            previous,
            expanded_capabilities,
            downgrade,
            source,
            trusted,
        })
    }

    pub fn install(
        &self,
        review: InstallReview,
        approval: InstallApproval,
    ) -> Result<InstalledPlugin, ManagerError> {
        if !review.expanded_capabilities.is_empty() && !approval.capability_expansion {
            return Err(ManagerError::CapabilityReviewRequired(
                review.expanded_capabilities,
            ));
        }
        if review.downgrade && !approval.downgrade {
            return Err(ManagerError::DowngradeConfirmationRequired);
        }

        let id = review.package.manifest.id.clone();
        let version = review.package.manifest.version.clone();
        let target = self.paths.version_root(&id, &version);
        if target.exists() {
            return Err(ManagerError::VersionAlreadyInstalled(version));
        }

        let staging = TempDir::new_in(&self.paths.staging).map_err(ManagerError::Io)?;
        write_package(staging.path(), &review.package)?;
        let staged_path = staging.keep();
        fs::create_dir_all(target.parent().unwrap()).map_err(ManagerError::Io)?;
        fs::rename(&staged_path, &target).map_err(ManagerError::Io)?;

        let previous = review.previous;
        let settings = previous.as_ref().map_or_else(
            || default_settings(&review.package.manifest),
            |plugin| {
                if plugin.settings_revision == 0 {
                    default_settings(&review.package.manifest)
                } else {
                    reconcile_settings(&review.package.manifest, &plugin.settings)
                }
            },
        );
        let grants = required_grants(
            &review.package.manifest,
            previous.as_ref().map(|plugin| &plugin.grants),
        );
        let mut installed = InstalledPlugin {
            manifest: review.package.manifest,
            enabled: previous.as_ref().map_or(true, |plugin| plugin.enabled),
            trusted: review.trusted,
            source: review.source,
            archive_hash: review.package.archive_hash,
            settings_revision: previous
                .as_ref()
                .map_or(0, |plugin| plugin.settings_revision),
            settings,
            grants,
        };
        if !review.expanded_capabilities.is_empty() {
            installed.enabled = false;
        }
        self.persist(&installed)?;
        Ok(installed)
    }

    pub fn inventory(&self) -> Result<Vec<InstalledPlugin>, ManagerError> {
        let entries = match fs::read_dir(&self.paths.packages) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(ManagerError::Io(error)),
        };
        let mut plugins = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path().join("state.json");
                read_json::<InstalledPlugin>(&path)
                    .transpose()
                    .map(|plugin| plugin.map(normalize_installed))
            })
            .collect::<Result<Vec<_>, _>>()?;
        plugins.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
        Ok(plugins)
    }

    pub fn installed(&self, id: &PluginId) -> Result<Option<InstalledPlugin>, ManagerError> {
        read_json(&self.paths.state_path(id)).map(|plugin| plugin.map(normalize_installed))
    }

    pub fn set_enabled(
        &self,
        id: &PluginId,
        enabled: bool,
    ) -> Result<InstalledPlugin, ManagerError> {
        let mut plugin = self
            .installed(id)?
            .ok_or_else(|| ManagerError::NotInstalled(id.clone()))?;
        plugin.enabled = enabled;
        self.persist(&plugin)?;
        Ok(plugin)
    }

    pub fn update_settings(
        &self,
        id: &PluginId,
        values: BTreeMap<String, serde_json::Value>,
    ) -> Result<InstalledPlugin, ManagerError> {
        let mut plugin = self
            .installed(id)?
            .ok_or_else(|| ManagerError::NotInstalled(id.clone()))?;
        plugin.settings = normalize_settings(&plugin.manifest, values)?;
        plugin.settings_revision = plugin.settings_revision.saturating_add(1);
        self.persist(&plugin)?;
        Ok(plugin)
    }

    pub fn reset_settings(&self, id: &PluginId) -> Result<InstalledPlugin, ManagerError> {
        let mut plugin = self
            .installed(id)?
            .ok_or_else(|| ManagerError::NotInstalled(id.clone()))?;
        plugin.settings = default_settings(&plugin.manifest);
        plugin.settings_revision = plugin.settings_revision.saturating_add(1);
        self.persist(&plugin)?;
        Ok(plugin)
    }

    pub fn set_grants(
        &self,
        id: &PluginId,
        grants: BTreeMap<Capability, BTreeSet<String>>,
    ) -> Result<InstalledPlugin, ManagerError> {
        let mut plugin = self
            .installed(id)?
            .ok_or_else(|| ManagerError::NotInstalled(id.clone()))?;
        plugin.grants = grants;
        self.persist(&plugin)?;
        Ok(plugin)
    }

    pub fn uninstall(&self, id: &PluginId, retain_data: bool) -> Result<(), ManagerError> {
        let root = self.paths.plugin_root(id);
        if !root.exists() {
            return Err(ManagerError::NotInstalled(id.clone()));
        }
        fs::remove_dir_all(root).map_err(ManagerError::Io)?;
        if !retain_data {
            let data = self.paths.data_root(id);
            if data.exists() {
                fs::remove_dir_all(data).map_err(ManagerError::Io)?;
            }
        }
        Ok(())
    }

    fn persist(&self, plugin: &InstalledPlugin) -> Result<(), ManagerError> {
        let path = self.paths.state_path(&plugin.manifest.id);
        fs::create_dir_all(path.parent().unwrap()).map_err(ManagerError::Io)?;
        write_json_atomic(&path, plugin)
    }
}

fn normalize_installed(mut plugin: InstalledPlugin) -> InstalledPlugin {
    plugin.settings = reconcile_settings(&plugin.manifest, &plugin.settings);
    plugin
}

fn required_grants(
    manifest: &PluginManifest,
    previous: Option<&BTreeMap<Capability, BTreeSet<String>>>,
) -> BTreeMap<Capability, BTreeSet<String>> {
    let mut grants = previous.cloned().unwrap_or_default();
    let declared = manifest
        .capabilities
        .iter()
        .map(|declaration| declaration.capability.clone())
        .collect::<BTreeSet<_>>();
    grants.retain(|capability, _| declared.contains(capability));
    for declaration in manifest
        .capabilities
        .iter()
        .filter(|declaration| !declaration.optional)
    {
        grants.insert(
            declaration.capability.clone(),
            declaration.scopes.iter().cloned().collect(),
        );
    }
    grants
}

fn write_package(root: &Path, package: &ValidatedPackage) -> Result<(), ManagerError> {
    for (relative, bytes) in &package.files {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).map_err(ManagerError::Io)?;
        fs::write(path, bytes).map_err(ManagerError::Io)?;
    }
    Ok(())
}

fn read_json<T>(path: &Path) -> Result<Option<T>, ManagerError>
where
    T: for<'de> Deserialize<'de>,
{
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(ManagerError::State),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ManagerError::Io(error)),
    }
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), ManagerError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(ManagerError::State)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(ManagerError::Io)?;
    temporary.write_all(&bytes).map_err(ManagerError::Io)?;
    temporary.as_file().sync_all().map_err(ManagerError::Io)?;
    temporary
        .persist(path)
        .map_err(|error| ManagerError::Io(error.error))?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error(transparent)]
    Package(#[from] crate::package::PackageError),
    #[error("plugin storage operation failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("plugin state is invalid: {0}")]
    State(#[source] serde_json::Error),
    #[error("plugin requests additional capabilities: {0:?}")]
    CapabilityReviewRequired(BTreeSet<Capability>),
    #[error("plugin downgrade requires confirmation")]
    DowngradeConfirmationRequired,
    #[error("plugin version is already installed: {0}")]
    VersionAlreadyInstalled(Version),
    #[error("plugin is not installed: {0}")]
    NotInstalled(PluginId),
    #[error("invalid plugin setting {key}: {detail}")]
    InvalidSetting { key: String, detail: String },
}
