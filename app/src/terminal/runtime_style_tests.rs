use super::scaled_tmux_surface_opacity;

#[test]
fn tmux_surface_opacity_is_eighty_percent_of_the_global_value() {
    assert_eq!(scaled_tmux_surface_opacity(100), 80);
    assert_eq!(scaled_tmux_surface_opacity(85), 68);
    assert_eq!(scaled_tmux_surface_opacity(0), 0);
}
