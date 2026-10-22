use std::collections::BTreeSet;

use warpeer_plugin_protocol::{
    ContainerExtension, ContainerRejection, ExtensionRequirement, ExtensionSupport,
};

pub fn negotiate_extensions(
    requirements: &[ExtensionRequirement],
) -> Result<Vec<ExtensionSupport>, ContainerRejection> {
    let mut seen = BTreeSet::new();
    let mut accepted = Vec::new();
    for requirement in requirements {
        if !requirement.is_valid() || !seen.insert(requirement.extension) {
            return Err(ContainerRejection::Compatibility(
                "invalid or duplicate extension requirement".into(),
            ));
        }
        if requirement.extension == ContainerExtension::Navigation {
            if requirement.optional {
                continue;
            }
            return Err(ContainerRejection::Compatibility(
                "page stack navigation is unavailable".into(),
            ));
        }
        let version = requirement.extension.host_version();
        if requirement.accepts(version) {
            accepted.push(ExtensionSupport {
                extension: requirement.extension,
                version,
            });
        } else if !requirement.optional {
            return Err(ContainerRejection::Compatibility(format!(
                "unsupported required extension {:?}",
                requirement.extension
            )));
        }
    }
    Ok(accepted)
}
