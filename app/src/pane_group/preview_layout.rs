use super::{PaneGroup, PaneId, SplitDirection};

#[derive(Clone, Debug)]
pub(crate) enum PanePreviewLayout {
    Pane(PaneId),
    Split {
        direction: SplitDirection,
        children: Vec<(f32, PanePreviewLayout)>,
    },
}

impl PaneGroup {
    pub(crate) fn preview_layout(&self) -> Option<PanePreviewLayout> {
        self.panes.preview_layout()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane_group::{Direction, PaneData, TerminalPaneId};

    fn pane_id() -> PaneId {
        TerminalPaneId::dummy_terminal_pane_id().into()
    }

    #[test]
    fn projection_preserves_nested_split_directions() {
        let first = pane_id();
        let second = pane_id();
        let third = pane_id();
        let mut panes = PaneData::new(first);
        assert!(panes.split(first, second, Direction::Right));
        assert!(panes.split(second, third, Direction::Down));

        let Some(PanePreviewLayout::Split {
            direction: SplitDirection::Horizontal,
            children,
        }) = panes.preview_layout()
        else {
            panic!("expected horizontal root preview");
        };
        assert!(matches!(&children[0].1, PanePreviewLayout::Pane(id) if *id == first));
        assert!(matches!(
            &children[1].1,
            PanePreviewLayout::Split {
                direction: SplitDirection::Vertical,
                ..
            }
        ));
    }

    #[test]
    fn projection_filters_hidden_panes_and_collapses_single_child_branches() {
        let first = pane_id();
        let second = pane_id();
        let mut panes = PaneData::new(first);
        assert!(panes.split(first, second, Direction::Right));
        panes.hide_pane_for_move(second);

        assert!(matches!(
            panes.preview_layout(),
            Some(PanePreviewLayout::Pane(id)) if id == first
        ));
    }
}
