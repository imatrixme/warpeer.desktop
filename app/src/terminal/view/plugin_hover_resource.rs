use super::GridHighlightedLink;
use crate::terminal::TerminalModel;
use crate::terminal::model::RespectObfuscatedSecrets;

pub(super) fn snapshot_grid_link(
    model: &TerminalModel,
    link: &GridHighlightedLink,
    respect_secrets: RespectObfuscatedSecrets,
) -> (String, Option<String>, bool) {
    match link {
        GridHighlightedLink::Url(link) => {
            let value = model.link_at_range(link, respect_secrets);
            (value.clone(), Some(value), false)
        }
        #[cfg(feature = "local_fs")]
        GridHighlightedLink::File(link) => {
            let value = link
                .get_inner()
                .absolute_path
                .to_string_lossy()
                .into_owned();
            (value.clone(), Some(value), false)
        }
        GridHighlightedLink::Hyperlink { uri, .. } => (uri.clone(), Some(uri.clone()), true),
    }
}

pub(super) fn is_supported_image_target(target: &str) -> bool {
    let target = target.to_ascii_lowercase();
    let path = target.split(['?', '#']).next().unwrap_or(&target);
    path.ends_with(".png") || path.ends_with(".jpg") || path.ends_with(".jpeg")
}
