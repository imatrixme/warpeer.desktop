use std::hash::{DefaultHasher, Hash, Hasher};

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;

use crate::editor::PlainTextEditorViewAction;

#[derive(Clone, Copy, Debug)]
pub(super) struct InputGlyphSnapshot {
    pub(super) hash: u64,
    pub(super) position: Vector2F,
    pub(super) scene_index: usize,
}

#[derive(Default)]
pub(super) struct InputGlyphChange {
    pub(super) inserted_scene_index: Option<usize>,
    pub(super) removed: Vec<InputGlyphSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InputEditDirection {
    Forward,
    Backward,
    Stationary,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InputCursorSnapshot {
    pub(super) bounds: RectF,
    pub(super) color: ColorU,
}

#[derive(Clone, Copy)]
pub(super) struct InputTextSnapshot {
    hash: u64,
    byte_len: usize,
    cursor_byte_offset: usize,
    action: Option<PlainTextEditorViewAction>,
    authoritative_previous_byte_len: Option<usize>,
}

impl InputTextSnapshot {
    pub(super) fn has_same_buffer_state(self, other: Self) -> bool {
        self.hash == other.hash
            && self.byte_len == other.byte_len
            && self.cursor_byte_offset == other.cursor_byte_offset
    }
}

pub(super) fn sorted_glyphs(glyphs: &[InputGlyphSnapshot]) -> Vec<InputGlyphSnapshot> {
    let mut sorted = glyphs.to_vec();
    sorted.sort_by(|left, right| {
        left.position
            .y()
            .total_cmp(&right.position.y())
            .then_with(|| left.position.x().total_cmp(&right.position.x()))
            .then_with(|| left.scene_index.cmp(&right.scene_index))
    });
    sorted
}

pub(super) fn same_glyph_sequence(
    left: &[InputGlyphSnapshot],
    right: &[InputGlyphSnapshot],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.hash == right.hash)
}

pub(super) fn reconcile_glyph_change(
    previous: &[InputGlyphSnapshot],
    current: &[InputGlyphSnapshot],
    cursor: Option<InputCursorSnapshot>,
    direction: InputEditDirection,
) -> InputGlyphChange {
    let mut prefix = 0;
    while prefix < previous.len()
        && prefix < current.len()
        && previous[prefix].hash == current[prefix].hash
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < previous.len().saturating_sub(prefix)
        && suffix < current.len().saturating_sub(prefix)
        && previous[previous.len() - 1 - suffix].hash == current[current.len() - 1 - suffix].hash
    {
        suffix += 1;
    }

    let inserted = &current[prefix..current.len().saturating_sub(suffix)];
    let removed = &previous[prefix..previous.len().saturating_sub(suffix)];
    InputGlyphChange {
        inserted_scene_index: (direction == InputEditDirection::Forward)
            .then(|| {
                cursor
                    .and_then(|cursor| glyph_before_cursor(inserted, cursor.bounds.origin()))
                    .or_else(|| inserted.last().map(|glyph| glyph.scene_index))
            })
            .flatten(),
        removed: if direction == InputEditDirection::Backward {
            cursor
                .and_then(|cursor| glyph_at_cursor(removed, cursor.bounds.origin()))
                .map(|glyph| vec![glyph])
                .unwrap_or_else(|| removed.iter().copied().rev().take(4).collect())
        } else {
            Vec::new()
        },
    }
}

pub(super) fn input_edit_direction(
    previous: Option<InputTextSnapshot>,
    current: InputTextSnapshot,
    composition_committed: bool,
    implicit_single_glyph_removal: bool,
) -> InputEditDirection {
    let authoritative_previous_byte_len = current
        .authoritative_previous_byte_len
        .filter(|byte_len| *byte_len > current.byte_len);
    let Some(previous) = previous else {
        if is_backward_action(current.action) && authoritative_previous_byte_len.is_some() {
            return InputEditDirection::Backward;
        }
        return InputEditDirection::Stationary;
    };
    if is_backward_action(current.action) && authoritative_previous_byte_len.is_some() {
        return InputEditDirection::Backward;
    }
    if current.hash == previous.hash && current.byte_len == previous.byte_len {
        return InputEditDirection::Stationary;
    }
    if composition_committed
        && current.byte_len > previous.byte_len
        && current.cursor_byte_offset >= previous.cursor_byte_offset
    {
        return InputEditDirection::Forward;
    }
    if matches!(
        current.action,
        Some(
            PlainTextEditorViewAction::ClearBuffer
                | PlainTextEditorViewAction::ReplaceBuffer
                | PlainTextEditorViewAction::AutoSuggestion
                | PlainTextEditorViewAction::AcceptCompletionSuggestion
                | PlainTextEditorViewAction::CycleCompletionSuggestion
                | PlainTextEditorViewAction::ExpandAlias
                | PlainTextEditorViewAction::UpdateMarkedText
        )
    ) {
        return InputEditDirection::Stationary;
    }
    match current.action {
        Some(
            PlainTextEditorViewAction::InsertChar
            | PlainTextEditorViewAction::SystemInsert
            | PlainTextEditorViewAction::Space
            | PlainTextEditorViewAction::NonExpandingSpace
            | PlainTextEditorViewAction::NewLine
            | PlainTextEditorViewAction::InsertSelectedText,
        ) if current.byte_len > previous.byte_len
            && current.cursor_byte_offset >= previous.cursor_byte_offset =>
        {
            InputEditDirection::Forward
        }
        action
            if is_backward_action(action)
                && current.byte_len
                    < authoritative_previous_byte_len.unwrap_or(previous.byte_len)
                && (authoritative_previous_byte_len.is_some()
                    || current.cursor_byte_offset <= previous.cursor_byte_offset) =>
        {
            InputEditDirection::Backward
        }
        None | Some(PlainTextEditorViewAction::CursorChanged)
            if current.byte_len < previous.byte_len
                && current.cursor_byte_offset <= previous.cursor_byte_offset
                && implicit_single_glyph_removal =>
        {
            InputEditDirection::Backward
        }
        _ => InputEditDirection::Stationary,
    }
}

pub(super) fn input_text_snapshot(
    text: &str,
    cursor_byte_offset: usize,
    action: Option<PlainTextEditorViewAction>,
) -> InputTextSnapshot {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    InputTextSnapshot {
        hash: hasher.finish(),
        byte_len: text.len(),
        cursor_byte_offset,
        action,
        authoritative_previous_byte_len: None,
    }
}

pub(super) fn input_text_snapshot_with_previous(
    text: &str,
    cursor_byte_offset: usize,
    action: Option<PlainTextEditorViewAction>,
    previous_text: &str,
) -> InputTextSnapshot {
    let mut snapshot = input_text_snapshot(text, cursor_byte_offset, action);
    snapshot.authoritative_previous_byte_len = Some(previous_text.len());
    snapshot
}

fn is_backward_action(action: Option<PlainTextEditorViewAction>) -> bool {
    matches!(
        action,
        Some(
            PlainTextEditorViewAction::Backspace
                | PlainTextEditorViewAction::Delete
                | PlainTextEditorViewAction::DeleteWordLeft
                | PlainTextEditorViewAction::DeleteWordRight
                | PlainTextEditorViewAction::DeleteAll
                | PlainTextEditorViewAction::CutWordLeft
                | PlainTextEditorViewAction::CutWordRight
                | PlainTextEditorViewAction::CutAll
                | PlainTextEditorViewAction::ClearAndCopyLines
                | PlainTextEditorViewAction::ClearLines
        )
    )
}

pub(super) fn input_glyph_before_cursor(
    glyphs: &[InputGlyphSnapshot],
    cursor: InputCursorSnapshot,
) -> Option<usize> {
    glyph_before_cursor(glyphs, cursor.bounds.origin())
}

fn glyph_before_cursor(glyphs: &[InputGlyphSnapshot], cursor: Vector2F) -> Option<usize> {
    glyphs
        .iter()
        .filter(|glyph| {
            (glyph.position.y() - cursor.y()).abs() < 24. && glyph.position.x() < cursor.x() - 0.25
        })
        .min_by(|left, right| {
            (cursor.x() - left.position.x())
                .abs()
                .total_cmp(&(cursor.x() - right.position.x()).abs())
        })
        .map(|glyph| glyph.scene_index)
}

fn glyph_at_cursor(glyphs: &[InputGlyphSnapshot], cursor: Vector2F) -> Option<InputGlyphSnapshot> {
    glyphs
        .iter()
        .copied()
        .filter(|glyph| {
            (glyph.position.y() - cursor.y()).abs() < 24. && glyph.position.x() >= cursor.x() - 0.25
        })
        .min_by(|left, right| {
            let left_distance = (left.position.x() - cursor.x()).abs();
            let right_distance = (right.position.x() - cursor.x()).abs();
            left_distance.total_cmp(&right_distance)
        })
        .or_else(|| {
            glyphs.iter().copied().min_by(|left, right| {
                let left_distance =
                    (left.position.x() - cursor.x()).abs() + (left.position.y() - cursor.y()).abs();
                let right_distance = (right.position.x() - cursor.x()).abs()
                    + (right.position.y() - cursor.y()).abs();
                left_distance.total_cmp(&right_distance)
            })
        })
}

pub(super) fn points_are_close(left: Vector2F, right: Vector2F) -> bool {
    (left.x() - right.x()).abs() < 0.25 && (left.y() - right.y()).abs() < 0.25
}

pub(super) fn is_single_glyph_removal(
    previous: &[InputGlyphSnapshot],
    current: &[InputGlyphSnapshot],
) -> bool {
    let mut prefix = 0;
    while prefix < previous.len()
        && prefix < current.len()
        && previous[prefix].hash == current[prefix].hash
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < previous.len().saturating_sub(prefix)
        && suffix < current.len().saturating_sub(prefix)
        && previous[previous.len() - 1 - suffix].hash == current[current.len() - 1 - suffix].hash
    {
        suffix += 1;
    }

    let removed = previous.len().saturating_sub(prefix + suffix);
    let inserted = current.len().saturating_sub(prefix + suffix);
    removed == 1 && inserted == 0
}
