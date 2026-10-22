use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::{ContentHash, Fragment, FragmentMetadata};
use crate::index::locations::CodeContextLocation;

#[derive(Default)]
pub struct ReadFragmentResult {
    pub successfully_read: Vec<Fragment>,
    pub fail_to_read: Vec<ContentHash>,
    pub fail_to_read_path: Vec<PathBuf>,
}

pub fn build_fragments_from_file_contents(
    metadatas: impl IntoIterator<Item = (ContentHash, FragmentMetadata)>,
    _file_contents: &HashMap<PathBuf, String>,
) -> ReadFragmentResult {
    let mut result = ReadFragmentResult::default();
    for (hash, metadata) in metadatas {
        result.fail_to_read.push(hash);
        result.fail_to_read_path.push(metadata.absolute_path);
    }
    result
}

pub fn fragments_to_context_locations<'a>(
    _fragments: Vec<Fragment>,
    _metadata_for_hash: impl Fn(&ContentHash) -> Option<&'a [FragmentMetadata]>,
    _context_lines: usize,
) -> HashSet<CodeContextLocation> {
    HashSet::new()
}
