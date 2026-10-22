use std::collections::HashSet;

use super::load;

#[test]
fn loads_fixed_ghostty_snapshot() {
    let themes = load().expect("the committed Ghostty theme snapshot should parse");
    assert_eq!(themes.len(), 602);
    assert!(themes.iter().any(|(id, _)| id == "Dracula"));
    assert!(themes.iter().any(|(id, _)| id == "Catppuccin Frappe"));

    let unique_ids = themes.iter().map(|(id, _)| id).collect::<HashSet<_>>();
    assert_eq!(unique_ids.len(), themes.len());
}
