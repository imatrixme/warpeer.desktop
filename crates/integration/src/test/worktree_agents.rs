use std::fs;

use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::integration_testing::worktree_agents::{
    assert_exact_resume_was_executed, open_claude_resume_in_new_tab,
};

use super::{Builder, new_builder};

pub fn test_warpeer_worktree_agent_native_resume() -> Builder {
    new_builder()
        .with_setup(|utils| {
            fs::create_dir_all(utils.test_dir().join("worktree-agent-control-center"))
                .expect("worktree fixture should be created");
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(open_claude_resume_in_new_tab("session-123"))
        .with_step(wait_until_bootstrapped_single_pane_for_tab(1))
        .with_step(assert_exact_resume_was_executed(1, "session-123"))
}
