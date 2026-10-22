use std::collections::HashMap;

use warpui::terminal_visuals::{TERMINAL_EFFECT_OFF, TerminalBlockMotion};

use super::{SemanticPulse, TerminalVisualRuntime};

impl TerminalVisualRuntime {
    pub(super) fn update_block_motions(&mut self) {
        if self.lane == super::RuntimeLane::StaticOutput
            || !self.config.enabled
            || self.config.reduce_motion
            || self.alternate_screen
            || self.config.profile.block_fold == TERMINAL_EFFECT_OFF
            || self.previous_blocks.is_empty()
        {
            return;
        }
        for (id, bounds) in &self.current_blocks {
            let Some(previous) = self.previous_blocks.get(id) else {
                continue;
            };
            if (previous.height() - bounds.height()).abs() <= 0.5 {
                continue;
            }
            let motion = TerminalBlockMotion {
                bounds: *bounds,
                start_seconds: self.now_seconds,
                effect_kind: self.config.profile.block_fold,
                phase: if bounds.height() > previous.height() {
                    1.
                } else {
                    -1.
                },
            };
            let end = self.now_seconds + self.config.duration_seconds;
            self.active_blocks.insert(*id, (motion, end));
            self.active_until_seconds = self.active_until_seconds.max(end);
        }
    }

    pub(super) fn frame_blocks(&self) -> Vec<TerminalBlockMotion> {
        self.current_blocks
            .iter()
            .map(|(id, bounds)| {
                if let Some((motion, _)) = self.active_blocks.get(id) {
                    return *motion;
                }
                let (start_seconds, effect_kind, phase) = match self.pulse {
                    Some((
                        SemanticPulse::BlockFold {
                            bounds: pulse,
                            expanding,
                        },
                        started,
                    )) if self.now_seconds - started <= self.config.duration_seconds
                        && pulse.intersection(*bounds).is_some() =>
                    {
                        (
                            started,
                            self.config.profile.block_fold,
                            if expanding { 1. } else { -1. },
                        )
                    }
                    _ => (self.now_seconds, TERMINAL_EFFECT_OFF, 0.),
                };
                TerminalBlockMotion {
                    bounds: *bounds,
                    start_seconds,
                    effect_kind,
                    phase,
                }
            })
            .collect()
    }

    pub(super) fn commit_block_snapshot(&mut self) {
        self.previous_blocks = self
            .current_blocks
            .iter()
            .copied()
            .collect::<HashMap<_, _>>();
    }
}
