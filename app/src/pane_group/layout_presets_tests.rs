use super::*;
use crate::pane_group::TerminalPaneId;

fn pane_ids(count: usize) -> Vec<PaneId> {
    (0..count)
        .map(|_| TerminalPaneId::dummy_terminal_pane_id().into())
        .collect()
}

#[test]
fn every_preset_builds_its_minimum_pane_count() {
    for preset in PaneLayoutPreset::ALL {
        let root = preset
            .build(&pane_ids(preset.pane_count()))
            .expect("preset should build");

        assert_eq!(root.pane_ids().len(), preset.pane_count());
    }
}

#[test]
fn presets_preserve_panes_beyond_their_minimum() {
    for preset in PaneLayoutPreset::ALL {
        let root = preset.build(&pane_ids(6)).expect("preset should build");

        assert_eq!(root.pane_ids().len(), 6);
    }
}

#[test]
fn presets_reject_too_few_panes() {
    assert!(PaneLayoutPreset::Grid.build(&pane_ids(3)).is_none());
}

#[test]
fn right_stack_uses_equal_columns_and_a_vertical_stack() {
    let root = PaneLayoutPreset::RightStack
        .build(&pane_ids(3))
        .expect("preset should build");
    let PaneNode::Branch(root) = root else {
        panic!("right stack should have a root branch");
    };

    assert_eq!(root.axis(), SplitDirection::Horizontal);
    assert_eq!(root.nodes[0].0.0, 1.);
    assert_eq!(root.nodes[1].0.0, 1.);
    let PaneNode::Branch(right) = &root.nodes[1].1 else {
        panic!("right side should be split");
    };
    assert_eq!(right.axis(), SplitDirection::Vertical);
}
