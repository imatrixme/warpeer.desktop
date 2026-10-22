use std::path::{Path, PathBuf};

use ai::skills::ParsedSkill;
use warp_cli::skill::SkillSpec;
use warpui::AppContext;

#[derive(Debug, Clone)]
pub struct ResolvedSkill {
    pub skill_path: PathBuf,
    pub name: String,
    pub instructions: String,
    pub parsed_skill: ParsedSkill,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveSkillError {
    #[error("Skill '{skill}' not found")]
    NotFound { skill: String },
    #[error("Repository '{repo}' not found")]
    RepoNotFound { repo: String },
    #[error("Skill '{skill}' is ambiguous; specify as repo:skill_name")]
    Ambiguous {
        skill: String,
        candidates: Vec<PathBuf>,
    },
    #[error("Repository '{repo}' found but belongs to org '{found}', expected '{expected}'")]
    OrgMismatch {
        repo: String,
        expected: String,
        found: String,
    },
    #[error("Failed to parse skill file {path}: {message}")]
    ParseFailed { path: PathBuf, message: String },
    #[error("Failed to clone repository '{org}/{repo}': {message}")]
    CloneFailed {
        org: String,
        repo: String,
        message: String,
    },
}

pub fn resolve_skill_spec(
    spec: &SkillSpec,
    working_dir: &Path,
    ctx: &AppContext,
) -> Result<ResolvedSkill, ResolveSkillError> {
    let _ = (working_dir, ctx);
    Err(ResolveSkillError::NotFound {
        skill: spec.skill_identifier.clone(),
    })
}

pub async fn clone_repo_for_skill(
    org: &str,
    repo: &str,
    working_dir: &Path,
) -> Result<(), ResolveSkillError> {
    let _ = working_dir;
    Err(ResolveSkillError::CloneFailed {
        org: org.to_string(),
        repo: repo.to_string(),
        message: "skills are unavailable in this product profile".to_string(),
    })
}
