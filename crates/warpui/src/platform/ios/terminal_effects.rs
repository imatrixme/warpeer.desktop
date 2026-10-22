use std::time::{Duration, Instant};

use super::{TerminalCell, TerminalDeletionEffect, TerminalInputEffect};

const MAX_ACTIVE_EFFECTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalEffectKind {
    Input(TerminalInputEffect),
    Deletion(TerminalDeletionEffect),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalVisualEffect {
    pub kind: TerminalEffectKind,
    pub cell: TerminalCell,
}

pub struct TerminalEffects {
    active: Vec<ActiveEffect>,
}

struct ActiveEffect {
    effect: TerminalVisualEffect,
    started_at: Instant,
    duration: Duration,
    lifetime: Duration,
}

#[derive(Clone, Copy)]
pub struct EffectSample<'a> {
    pub effect: &'a TerminalVisualEffect,
    pub progress: f32,
}

impl TerminalEffects {
    pub fn new() -> Self {
        Self { active: Vec::new() }
    }

    pub fn push(&mut self, effect: TerminalVisualEffect, duration_ms: f32, reduced_motion: bool) {
        if is_off(effect.kind) {
            return;
        }
        let effect = if reduced_motion {
            TerminalVisualEffect {
                kind: match effect.kind {
                    TerminalEffectKind::Input(_) => {
                        TerminalEffectKind::Input(TerminalInputEffect::Fade)
                    }
                    TerminalEffectKind::Deletion(_) => {
                        TerminalEffectKind::Deletion(TerminalDeletionEffect::Dissolve)
                    }
                },
                cell: effect.cell,
            }
        } else {
            effect
        };
        if self.active.len() == MAX_ACTIVE_EFFECTS {
            self.active.remove(0);
        }
        let duration = Duration::from_secs_f32(duration_ms.clamp(80.0, 600.0) / 1_000.0);
        let lifetime = if matches!(
            effect.kind,
            TerminalEffectKind::Deletion(TerminalDeletionEffect::Shatter)
        ) {
            duration.mul_f32(2.12)
        } else {
            duration
        };
        self.active.push(ActiveEffect {
            effect,
            started_at: Instant::now(),
            duration,
            lifetime,
        });
    }

    pub fn clear(&mut self) {
        self.active.clear();
    }

    pub fn advance(&mut self, now: Instant) -> bool {
        let previous_len = self.active.len();
        self.active
            .retain(|effect| now.duration_since(effect.started_at) < effect.lifetime);
        !self.active.is_empty() || previous_len != self.active.len()
    }

    pub fn is_active(&self) -> bool {
        !self.active.is_empty()
    }

    pub fn input_at(&self, row: u16, column: u16, now: Instant) -> Option<EffectSample<'_>> {
        self.active.iter().rev().find_map(|effect| {
            matches!(effect.effect.kind, TerminalEffectKind::Input(_))
                .then(|| sample(effect, now))
                .filter(|sample| {
                    sample.effect.cell.row == row && sample.effect.cell.column == column
                })
        })
    }

    pub fn deletion_samples(&self, now: Instant) -> impl Iterator<Item = EffectSample<'_>> {
        self.active.iter().filter_map(move |effect| {
            matches!(effect.effect.kind, TerminalEffectKind::Deletion(_))
                .then(|| sample(effect, now))
        })
    }
}

fn sample(effect: &ActiveEffect, now: Instant) -> EffectSample<'_> {
    EffectSample {
        effect: &effect.effect,
        progress: (now.duration_since(effect.started_at).as_secs_f32()
            / effect.duration.as_secs_f32())
        .clamp(0.0, 1.0),
    }
}

fn is_off(kind: TerminalEffectKind) -> bool {
    matches!(
        kind,
        TerminalEffectKind::Input(TerminalInputEffect::Off)
            | TerminalEffectKind::Deletion(TerminalDeletionEffect::Off)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_effects_are_bounded_and_clearable() {
        let cell = TerminalCell {
            row: 0,
            column: 0,
            text: "a".to_owned(),
            foreground: [255; 4],
            background: [0, 0, 0, 255],
            wide: false,
            underline: false,
        };
        let mut effects = TerminalEffects::new();
        for _ in 0..(MAX_ACTIVE_EFFECTS + 5) {
            effects.push(
                TerminalVisualEffect {
                    kind: TerminalEffectKind::Input(TerminalInputEffect::Pop),
                    cell: cell.clone(),
                },
                180.0,
                false,
            );
        }
        assert_eq!(effects.active.len(), MAX_ACTIVE_EFFECTS);
        effects.clear();
        assert!(!effects.is_active());
    }

    #[test]
    fn reduced_motion_replaces_spatial_effects() {
        let mut effects = TerminalEffects::new();
        effects.push(
            TerminalVisualEffect {
                kind: TerminalEffectKind::Deletion(TerminalDeletionEffect::Shatter),
                cell: TerminalCell {
                    row: 0,
                    column: 0,
                    text: "a".to_owned(),
                    foreground: [255; 4],
                    background: [0, 0, 0, 255],
                    wide: false,
                    underline: false,
                },
            },
            180.0,
            true,
        );
        assert!(matches!(
            effects.active[0].effect.kind,
            TerminalEffectKind::Deletion(TerminalDeletionEffect::Dissolve)
        ));
        assert_eq!(effects.active[0].duration, effects.active[0].lifetime);
    }

    #[test]
    fn shatter_keeps_the_desktop_particle_tail_alive() {
        let mut effects = TerminalEffects::new();
        effects.push(
            TerminalVisualEffect {
                kind: TerminalEffectKind::Deletion(TerminalDeletionEffect::Shatter),
                cell: TerminalCell {
                    row: 0,
                    column: 0,
                    text: "a".to_owned(),
                    foreground: [255; 4],
                    background: [0, 0, 0, 255],
                    wide: false,
                    underline: false,
                },
            },
            220.0,
            false,
        );

        assert!(effects.active[0].lifetime > effects.active[0].duration.mul_f32(2.0));
    }
}
