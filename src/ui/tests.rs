use super::*;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_uses_zed_like_compact_scale_by_default() {
        let spacing = Spacing::default();
        assert_eq!(spacing.base04(), px(2.0));
        assert_eq!(spacing.base08(), px(4.0));
        assert_eq!(spacing.base16(), px(6.0));
        assert_eq!(spacing.fixed32(), px(32.0));
    }

    #[test]
    fn spacing_can_still_use_zed_like_default_scale() {
        let spacing = Spacing::new(UiDensity::Default);
        assert_eq!(spacing.base04(), px(2.0));
        assert_eq!(spacing.base08(), px(4.0));
        assert_eq!(spacing.base16(), px(8.0));
    }

    #[test]
    fn spacing_exposes_layout_gaps_separate_from_padding() {
        let spacing = Spacing::default();
        assert_eq!(spacing.control_gap(), px(4.0));
        assert_eq!(spacing.cluster_gap(), px(5.0));
        assert_eq!(spacing.component_gap(), px(10.0));
        assert_eq!(spacing.section_gap(), px(14.0));
    }

    #[test]
    fn button_sizes_match_expected_heights() {
        assert_eq!(ButtonSize::Large.height(), px(32.0));
        assert_eq!(ButtonSize::Medium.height(), px(28.0));
        assert_eq!(ButtonSize::Default.height(), px(22.0));
        assert_eq!(ButtonSize::Compact.height(), px(18.0));
    }
}
