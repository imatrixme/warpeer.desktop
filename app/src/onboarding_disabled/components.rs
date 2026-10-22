pub mod feature_optout_dialog {
    use warp_core::ui::appearance::Appearance;
    use warpui::Element;
    use warpui::elements::{Empty, ParentElement};

    pub struct FeatureOptOutDialog {
        pub title: &'static str,
        pub body: &'static str,
        pub features: &'static [&'static str],
        pub close_button: Box<dyn Element>,
        pub cancel_button: Box<dyn Element>,
        pub confirm_button: Box<dyn Element>,
    }

    pub fn render_feature_optout_dialog(
        appearance: &Appearance,
        dialog: FeatureOptOutDialog,
    ) -> Box<dyn Element> {
        let _ = appearance;
        drop(dialog);
        Empty::new().finish()
    }
}
