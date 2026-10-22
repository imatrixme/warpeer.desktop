use warpui::keymap::Context;

use super::{
    terminal_alt_screen_cursor_bound_context_predicate, terminal_cursor_bound_context_predicate,
};

#[test]
fn control_character_cursor_bound_shortcuts_do_not_apply_to_alt_screen_terminals() {
    let predicate = terminal_cursor_bound_context_predicate();
    let mut context = Context::default();
    context.set.insert("Terminal");

    assert!(!predicate.eval(&context));

    context.set.insert("LongRunningCommand");
    assert!(predicate.eval(&context));

    context.set.insert("AltScreen");
    assert!(!predicate.eval(&context));

    context.set.insert("IMEOpen");
    assert!(!predicate.eval(&context));
}

#[test]
fn alt_screen_cursor_bound_shortcuts_use_terminal_home_and_end_actions() {
    let predicate = terminal_alt_screen_cursor_bound_context_predicate();
    let mut context = Context::default();
    context.set.insert("Terminal");

    assert!(!predicate.eval(&context));

    context.set.insert("AltScreen");
    assert!(predicate.eval(&context));

    context.set.insert("IMEOpen");
    assert!(!predicate.eval(&context));
}
