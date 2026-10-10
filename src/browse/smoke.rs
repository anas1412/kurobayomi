//! The GTK checks of the browse widgets. Every widget check of this lane runs inside this one test, on the shared GTK
//! thread of `crate::testing`; it skips itself when there is no display.

#[test]
fn browse_widgets_smoke() {
    crate::testing::with_display(|| {
        super::view::tests::smoke();
        super::dialogs::tests::smoke();
        super::filter_sheet::tests::smoke();
        super::favorite_flow::tests::smoke();
        super::header_bar::tests::smoke();
        super::sources_page::tests::smoke();
        super::extensions_page::tests::smoke();
        super::lang_pages::tests::smoke();
        super::ext_details_page::tests::smoke();
        super::source_prefs_page::tests::smoke();
        super::shell::tests::smoke();
        super::pages::tests::smoke();
        super::listing_view::tests::smoke();
        super::browse_screen::tests::smoke();
        super::search_screen::tests::smoke();
        super::migrate_nav::tests::smoke();
        super::migrate_list::tests::smoke();
        super::spec_tests::smoke();
        super::anime::sources_view::tests::smoke();
        super::anime::browse_view::tests::smoke();
    });
}
