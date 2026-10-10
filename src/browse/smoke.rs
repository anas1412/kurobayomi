//! The GTK checks of the browse widgets. GTK binds to the first thread that starts it, and `cargo test` runs tests on
//! threads of their own, so every widget check of this lane runs inside this one test, on the thread that starts GTK.
//! It needs a display, so it is ignored by default: `cargo test -p kurobayomi browse_widgets_smoke -- --ignored`
//! (it skips itself when there is no display).

#[test]
#[ignore = "needs a display; run alone: cargo test -p kurobayomi browse_widgets_smoke -- --ignored"]
fn browse_widgets_smoke() {
    if gtk::init().is_err() {
        eprintln!("no display: skipping the browse widget checks");
        return;
    }
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
    super::migrate_nav::tests::smoke();
    super::spec_tests::smoke();
    super::anime::sources_view::tests::smoke();
    super::anime::browse_view::tests::smoke();
}
