use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};

use semver::Version;
use sha2::{Digest, Sha256};
use warpeer_plugin_protocol::{
    CURRENT_PROTOCOL_VERSION, CURRENT_RUNTIME_ABI, Capability, ContainerExtension, PluginManifest,
    SettingValueDeclaration,
};
use zip::ZipArchive;

pub const PACKAGE_EXTENSION: &str = "warpeerplugin";
pub const MANIFEST_PATH: &str = "plugin.toml";
const MAX_ARCHIVE_ENTRIES: usize = 512;
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ValidatedPackage {
    pub manifest: PluginManifest,
    pub files: BTreeMap<PathBuf, Vec<u8>>,
    pub archive_hash: String,
}

impl ValidatedPackage {
    pub fn file(&self, path: impl AsRef<Path>) -> Option<&[u8]> {
        self.files.get(path.as_ref()).map(Vec::as_slice)
    }
}

pub fn validate_package(path: &Path) -> Result<ValidatedPackage, PackageError> {
    if path.extension().and_then(|value| value.to_str()) != Some(PACKAGE_EXTENSION) {
        return Err(PackageError::WrongExtension);
    }
    let file = File::open(path).map_err(PackageError::Open)?;
    validate_archive(file)
}

pub fn validate_archive<R>(mut reader: R) -> Result<ValidatedPackage, PackageError>
where
    R: Read + Seek,
{
    let archive_hash = hash_reader(&mut reader)?;
    reader.rewind().map_err(PackageError::Read)?;
    let mut archive = ZipArchive::new(reader).map_err(PackageError::Archive)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(PackageError::TooManyEntries(archive.len()));
    }

    let mut files = BTreeMap::new();
    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(PackageError::Archive)?;
        let raw_name = entry.name().to_owned();
        let path = safe_relative_path(&raw_name)?;
        if entry.is_dir() {
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(PackageError::Symlink { path });
        }
        if entry.size() > MAX_FILE_BYTES {
            return Err(PackageError::FileTooLarge { path });
        }
        total_bytes = total_bytes.saturating_add(entry.size());
        if total_bytes > MAX_ARCHIVE_BYTES {
            return Err(PackageError::ArchiveTooLarge);
        }
        if contains_native_payload(&path) {
            return Err(PackageError::NativePayload { path });
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(PackageError::Read)?;
        if files.insert(path.clone(), bytes).is_some() {
            return Err(PackageError::DuplicateEntry { path });
        }
    }

    let manifest_bytes = files
        .get(Path::new(MANIFEST_PATH))
        .ok_or(PackageError::MissingManifest)?;
    let manifest_text = std::str::from_utf8(manifest_bytes).map_err(PackageError::ManifestUtf8)?;
    let manifest =
        toml::from_str::<PluginManifest>(manifest_text).map_err(PackageError::Manifest)?;
    validate_manifest(&manifest, &files)?;

    Ok(ValidatedPackage {
        manifest,
        files,
        archive_hash,
    })
}

fn validate_manifest(
    manifest: &PluginManifest,
    files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), PackageError> {
    if manifest.name.trim().is_empty() || manifest.description.trim().is_empty() {
        return Err(PackageError::InvalidManifest(
            "name and description are required".into(),
        ));
    }
    if !manifest
        .runtime
        .accepts(CURRENT_PROTOCOL_VERSION.0, CURRENT_RUNTIME_ABI)
    {
        return Err(PackageError::IncompatibleRuntime {
            protocol: manifest.runtime.protocol,
            abi: manifest.runtime.abi.to_string(),
        });
    }

    let bundle = safe_relative_path(&manifest.entrypoints.bundle)?;
    let assets = safe_relative_path(&manifest.entrypoints.assets)?;
    for required in [&bundle, &assets] {
        if !files.contains_key(required) {
            return Err(PackageError::MissingEntrypoint {
                path: required.clone(),
            });
        }
    }
    if let Some(icon) = &manifest.entrypoints.icon {
        let icon = safe_relative_path(icon)?;
        if !files.contains_key(&icon) {
            return Err(PackageError::MissingEntrypoint { path: icon });
        }
    }

    let declared = manifest
        .hashes
        .keys()
        .map(|path| safe_relative_path(path))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut allowed = declared.clone();
    allowed.insert(PathBuf::from(MANIFEST_PATH));
    if let Some(signature) = &manifest.signature {
        allowed.insert(safe_relative_path(&signature.file)?);
    }
    for path in files.keys() {
        if !allowed.contains(path) {
            return Err(PackageError::UndeclaredFile { path: path.clone() });
        }
    }
    for (raw_path, expected) in &manifest.hashes {
        let path = safe_relative_path(raw_path)?;
        let bytes = files
            .get(&path)
            .ok_or_else(|| PackageError::MissingDeclaredFile { path: path.clone() })?;
        let actual = hex_digest(bytes);
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(PackageError::HashMismatch {
                path,
                expected: expected.clone(),
                actual,
            });
        }
    }

    let mut capability_names = BTreeSet::new();
    for declaration in &manifest.capabilities {
        if declaration.reason.trim().is_empty() {
            return Err(PackageError::InvalidManifest(format!(
                "capability {:?} requires a reason",
                declaration.capability
            )));
        }
        if !capability_names.insert(declaration.capability.clone()) {
            return Err(PackageError::InvalidManifest(format!(
                "capability {:?} is declared more than once",
                declaration.capability
            )));
        }
    }
    if !manifest.surfaces.is_empty() && !capability_names.contains(&Capability::PaneSurface) {
        return Err(PackageError::InvalidManifest(
            "surface plugins must declare pane-surface capability".into(),
        ));
    }
    if !manifest.triggers.is_empty() && !capability_names.contains(&Capability::TerminalContext) {
        return Err(PackageError::InvalidManifest(
            "terminal hover triggers must declare terminal-context capability".into(),
        ));
    }
    let mut extension_names = BTreeSet::new();
    for requirement in &manifest.extensions {
        if !requirement.is_valid() || !extension_names.insert(requirement.extension) {
            return Err(PackageError::InvalidManifest(
                "container extensions must be valid and unique".into(),
            ));
        }
        let host_version = requirement.extension.host_version();
        if !requirement.optional && !requirement.accepts(host_version) {
            return Err(PackageError::IncompatibleExtension {
                extension: requirement.extension,
                minimum: requirement.min_version,
                maximum: requirement.max_version,
                available: host_version,
            });
        }
    }
    let mut setting_keys = BTreeSet::new();
    for setting in &manifest.settings {
        if setting.key.trim().is_empty()
            || setting.title.trim().is_empty()
            || setting.description.trim().is_empty()
            || !setting_keys.insert(setting.key.clone())
        {
            return Err(PackageError::InvalidManifest(
                "plugin settings require unique keys, titles, and descriptions".into(),
            ));
        }
        if let SettingValueDeclaration::Integer {
            default,
            min,
            max,
            step,
            ..
        } = setting.value
            && (min >= max
                || default < min
                || default > max
                || step <= 0
                || (default - min) % step != 0)
        {
            return Err(PackageError::InvalidManifest(format!(
                "integer setting {} has invalid bounds, default, or step",
                setting.key
            )));
        }
    }
    let budgets = manifest.budgets;
    if budgets.memory_bytes == 0
        || budgets.execution_slice_ms == 0
        || budgets.queued_messages == 0
        || budgets.active_surfaces == 0
        || budgets.max_surface_pixels == 0
        || budgets.max_frame_rate == 0
    {
        return Err(PackageError::InvalidManifest(
            "worker and surface budgets must be greater than zero".into(),
        ));
    }
    if budgets.cache_bytes > budgets.memory_bytes {
        return Err(PackageError::InvalidManifest(
            "cache budget cannot exceed worker memory budget".into(),
        ));
    }
    Ok(())
}

pub fn expanded_capabilities(
    previous: &PluginManifest,
    next: &PluginManifest,
) -> BTreeSet<Capability> {
    let old = previous
        .capabilities
        .iter()
        .map(|declaration| declaration.capability.clone())
        .collect::<BTreeSet<_>>();
    next.capabilities
        .iter()
        .map(|declaration| declaration.capability.clone())
        .filter(|capability| !old.contains(capability))
        .collect()
}

pub fn is_downgrade(previous: &Version, next: &Version) -> bool {
    next < previous
}

fn safe_relative_path(raw: &str) -> Result<PathBuf, PackageError> {
    if raw.is_empty() || raw.contains('\0') || raw.contains('\\') {
        return Err(PackageError::UnsafePath(raw.into()));
    }
    let path = Path::new(raw);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(PackageError::UnsafePath(raw.into()));
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            Component::CurDir => None,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => None,
        })
        .collect::<PathBuf>();
    if normalized.as_os_str().is_empty() {
        return Err(PackageError::UnsafePath(raw.into()));
    }
    Ok(normalized)
}

fn contains_native_payload(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("dylib" | "so" | "dll" | "exe" | "app" | "framework")
    )
}

fn hash_reader<R: Read + Seek>(reader: &mut R) -> Result<String, PackageError> {
    let mut hasher = Sha256::new();
    std::io::copy(reader, &mut hasher).map_err(PackageError::Read)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("plugin package must use the .warpeerplugin extension")]
    WrongExtension,
    #[error("failed to open plugin package: {0}")]
    Open(#[source] std::io::Error),
    #[error("failed to read plugin package: {0}")]
    Read(#[source] std::io::Error),
    #[error("invalid plugin archive: {0}")]
    Archive(#[source] zip::result::ZipError),
    #[error("plugin archive has {0} entries; maximum is {MAX_ARCHIVE_ENTRIES}")]
    TooManyEntries(usize),
    #[error("plugin archive exceeds the uncompressed size limit")]
    ArchiveTooLarge,
    #[error("plugin file exceeds the size limit: {path}", path = .path.display())]
    FileTooLarge { path: PathBuf },
    #[error("unsafe plugin archive path: {0}")]
    UnsafePath(String),
    #[error("plugin archive contains a symlink: {path}", path = .path.display())]
    Symlink { path: PathBuf },
    #[error("plugin archive contains duplicate entry: {path}", path = .path.display())]
    DuplicateEntry { path: PathBuf },
    #[error("plugin archive contains unsupported native payload: {path}", path = .path.display())]
    NativePayload { path: PathBuf },
    #[error("plugin.toml is missing")]
    MissingManifest,
    #[error("plugin.toml is not UTF-8: {0}")]
    ManifestUtf8(#[source] std::str::Utf8Error),
    #[error("plugin.toml is invalid: {0}")]
    Manifest(#[source] toml::de::Error),
    #[error("plugin manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("plugin requires protocol {protocol} and ABI {abi}")]
    IncompatibleRuntime { protocol: u16, abi: String },
    #[error(
        "plugin requires {extension:?} extension versions {minimum} through {maximum}; available version is {available}"
    )]
    IncompatibleExtension {
        extension: ContainerExtension,
        minimum: u16,
        maximum: u16,
        available: u16,
    },
    #[error("plugin entrypoint is missing: {path}", path = .path.display())]
    MissingEntrypoint { path: PathBuf },
    #[error("plugin package contains undeclared file: {path}", path = .path.display())]
    UndeclaredFile { path: PathBuf },
    #[error("plugin manifest declares a missing file: {path}", path = .path.display())]
    MissingDeclaredFile { path: PathBuf },
    #[error("plugin file hash mismatch for {path}: expected {expected}, got {actual}", path = .path.display())]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
}
