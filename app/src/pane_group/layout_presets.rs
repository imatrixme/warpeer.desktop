use super::{PaneFlex, PaneId, PaneNode, SplitDirection};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PaneLayoutPreset {
    TwoColumns,
    TwoRows,
    ThreeColumns,
    ThreeRows,
    RightStack,
    LeftStack,
    BottomStack,
    TopStack,
    MainLeft,
    MainTop,
    Grid,
}

impl PaneLayoutPreset {
    pub const ALL: [Self; 11] = [
        Self::TwoColumns,
        Self::TwoRows,
        Self::ThreeColumns,
        Self::ThreeRows,
        Self::RightStack,
        Self::LeftStack,
        Self::BottomStack,
        Self::TopStack,
        Self::MainLeft,
        Self::MainTop,
        Self::Grid,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::TwoColumns => "Two columns",
            Self::TwoRows => "Two rows",
            Self::ThreeColumns => "Three columns",
            Self::ThreeRows => "Three rows",
            Self::RightStack => "Split right",
            Self::LeftStack => "Split left",
            Self::BottomStack => "Split bottom",
            Self::TopStack => "Split top",
            Self::MainLeft => "Main left",
            Self::MainTop => "Main top",
            Self::Grid => "Four pane grid",
        }
    }

    pub fn pane_count(self) -> usize {
        match self {
            Self::TwoColumns | Self::TwoRows => 2,
            Self::ThreeColumns
            | Self::ThreeRows
            | Self::RightStack
            | Self::LeftStack
            | Self::BottomStack
            | Self::TopStack
            | Self::MainLeft
            | Self::MainTop => 3,
            Self::Grid => 4,
        }
    }

    pub(crate) fn build(self, pane_ids: &[PaneId]) -> Option<PaneNode> {
        if pane_ids.len() < self.pane_count() {
            return None;
        }

        let leaf = |index| PaneNode::Leaf(pane_ids[index]);

        Some(match self {
            Self::TwoColumns => grouped(
                pane_ids,
                SplitDirection::Horizontal,
                SplitDirection::Vertical,
                2,
            ),
            Self::TwoRows => grouped(
                pane_ids,
                SplitDirection::Vertical,
                SplitDirection::Horizontal,
                2,
            ),
            Self::ThreeColumns => grouped(
                pane_ids,
                SplitDirection::Horizontal,
                SplitDirection::Vertical,
                3,
            ),
            Self::ThreeRows => grouped(
                pane_ids,
                SplitDirection::Vertical,
                SplitDirection::Horizontal,
                3,
            ),
            Self::RightStack => PaneNode::branch(
                SplitDirection::Horizontal,
                vec![
                    (PaneFlex(1.), leaf(0)),
                    (
                        PaneFlex(1.),
                        stack(pane_ids, SplitDirection::Vertical, 1..pane_ids.len()),
                    ),
                ],
            ),
            Self::LeftStack => PaneNode::branch(
                SplitDirection::Horizontal,
                vec![
                    (
                        PaneFlex(1.),
                        stack(pane_ids, SplitDirection::Vertical, 0..pane_ids.len() - 1),
                    ),
                    (PaneFlex(1.), leaf(pane_ids.len() - 1)),
                ],
            ),
            Self::BottomStack => PaneNode::branch(
                SplitDirection::Vertical,
                vec![
                    (PaneFlex(1.), leaf(0)),
                    (
                        PaneFlex(1.),
                        stack(pane_ids, SplitDirection::Horizontal, 1..pane_ids.len()),
                    ),
                ],
            ),
            Self::TopStack => PaneNode::branch(
                SplitDirection::Vertical,
                vec![
                    (
                        PaneFlex(1.),
                        stack(pane_ids, SplitDirection::Horizontal, 0..pane_ids.len() - 1),
                    ),
                    (PaneFlex(1.), leaf(pane_ids.len() - 1)),
                ],
            ),
            Self::MainLeft => PaneNode::branch(
                SplitDirection::Horizontal,
                vec![
                    (PaneFlex(2.), leaf(0)),
                    (
                        PaneFlex(1.),
                        stack(pane_ids, SplitDirection::Vertical, 1..pane_ids.len()),
                    ),
                ],
            ),
            Self::MainTop => PaneNode::branch(
                SplitDirection::Vertical,
                vec![
                    (PaneFlex(2.), leaf(0)),
                    (
                        PaneFlex(1.),
                        stack(pane_ids, SplitDirection::Horizontal, 1..pane_ids.len()),
                    ),
                ],
            ),
            Self::Grid => grouped(
                pane_ids,
                SplitDirection::Horizontal,
                SplitDirection::Vertical,
                2,
            ),
        })
    }
}

fn grouped(
    pane_ids: &[PaneId],
    direction: SplitDirection,
    child_direction: SplitDirection,
    group_count: usize,
) -> PaneNode {
    let group_size = pane_ids.len() / group_count;
    let remainder = pane_ids.len() % group_count;
    let mut start = 0;
    let mut children = Vec::with_capacity(group_count);
    for group_index in 0..group_count {
        let end = start + group_size + usize::from(group_index < remainder);
        children.push((PaneFlex(1.), stack(pane_ids, child_direction, start..end)));
        start = end;
    }

    PaneNode::branch(direction, children)
}

fn stack(
    pane_ids: &[PaneId],
    direction: SplitDirection,
    indices: std::ops::Range<usize>,
) -> PaneNode {
    if indices.len() == 1 {
        return PaneNode::Leaf(pane_ids[indices.start]);
    }

    PaneNode::branch(
        direction,
        indices
            .map(|index| (PaneFlex(1.), PaneNode::Leaf(pane_ids[index])))
            .collect(),
    )
}

#[cfg(test)]
#[path = "layout_presets_tests.rs"]
mod tests;
