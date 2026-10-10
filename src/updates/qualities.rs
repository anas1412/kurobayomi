//! SHOW_QUALITIES of an anime unit (T-42b-009): the episode options sheet. The sheet belongs to the player area; it
//! registers here with [`install`], and the Updates feed asks for it with [`open`] for exactly one row (E-U49). Without
//! a sheet [`available`] is `false`, and the feed does not offer the action.

use std::cell::RefCell;
use std::rc::Rc;

use crate::updates::download_actions::{self, UnitTarget};
use crate::updates::feed::FeedRow;

/// What the sheet needs to list the qualities of one episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualityRequest {
    pub episode_title: String,
    pub unit_id: i64,
    pub entry_id: i64,
    pub source_id: i64,
    /// The anime "use external downloader" choice of the screen.
    pub external_downloader: bool,
}

thread_local! {
    static SHEET: RefCell<Option<Rc<dyn Fn(QualityRequest)>>> = const { RefCell::new(None) };
}

/// Registers the options sheet. Replaces an earlier one.
pub fn install(sheet: impl Fn(QualityRequest) + 'static) {
    SHEET.with(|s| *s.borrow_mut() = Some(Rc::new(sheet)));
}

/// Whether a sheet is registered.
pub fn available() -> bool {
    SHEET.with(|s| s.borrow().is_some())
}

/// The request for the one selected row; `None` for any other count (the selection is then left as it is) or when the
/// row is not in `rows`.
pub fn request_of(targets: &[UnitTarget], rows: &[FeedRow], external_downloader: bool) -> Option<QualityRequest> {
    let target = download_actions::quality_target(targets)?;
    let row = rows.iter().find(|r| r.row.item_id == target.unit_id)?;
    Some(QualityRequest {
        episode_title: row.row.item_name.clone(),
        unit_id: target.unit_id,
        entry_id: target.entry_id,
        source_id: row.row.source_id,
        external_downloader,
    })
}

/// Opens the sheet for `request`. `false` when no sheet is registered.
pub fn open(request: QualityRequest) -> bool {
    match SHEET.with(|s| s.borrow().clone()) {
        Some(sheet) => {
            sheet(request);
            true
        }
        None => false,
    }
}

/// The spec screen and dialog of the episode options.
const OPTIONS_SCREEN: &str = "anime_entry_details";
const OPTIONS_DIALOG: &str = "episode_options";

/// The episode options dialog for `request`: the spec dialog under the episode's title (the dialog's own title is the
/// episode title, `EpisodeOptionsDialogScreen`).
pub fn episode_dialog(spec: &crate::spec::Dialog, request: &QualityRequest) -> crate::spec::Dialog {
    crate::spec::Dialog { title: request.episode_title.clone(), ..spec.clone() }
}

/// Registers the episode options dialog of the anime entry screen as the sheet (T-42a-037, T-42b-019). Called when the
/// Updates page is built, so the row menu offers Show qualities from the first row.
pub fn install_episode_options() {
    install(|request| {
        tracing::debug!(
            episode = request.unit_id,
            anime = request.entry_id,
            source = request.source_id,
            external = request.external_downloader,
            "opening the episode options"
        );
        match crate::ui::screen(OPTIONS_SCREEN).dialogs.iter().find(|d| d.id == OPTIONS_DIALOG) {
            Some(spec) => crate::ui::show_dialog(&episode_dialog(spec, &request)),
            None => tracing::warn!("the episode options dialog is not in the spec"),
        }
    });
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use kuro_domain::kind::EntryKind;
    use kuro_domain::updates::model::UpdateRow;

    use super::*;
    use crate::updates::download_state::DownloadState;
    use crate::updates::feed::NOT_DOWNLOADED;

    fn row(id: i64) -> FeedRow {
        FeedRow {
            row: UpdateRow {
                kind: EntryKind::Anime,
                entry_id: 4,
                entry_title: "T".into(),
                item_id: id,
                item_name: format!("Episode {id}"),
                scanlator: None,
                done: false,
                bookmark: false,
                fillermark: false,
                progress: 0,
                total_seconds: 0,
                source_id: 9,
                date_fetch: 0,
                date_upload: 0,
                thumbnail_url: None,
                cover_last_modified: 0,
            },
            selected: true,
            download: NOT_DOWNLOADED,
        }
    }

    fn target(id: i64) -> UnitTarget {
        UnitTarget { entry_id: 4, unit_id: id, state: DownloadState::NotDownloaded }
    }

    #[test]
    fn exactly_one_row_opens_the_sheet_with_its_episode() {
        let rows = [row(1), row(2)];
        let request = request_of(&[target(2)], &rows, true).expect("one row");
        assert_eq!(request, QualityRequest { episode_title: "Episode 2".into(), unit_id: 2, entry_id: 4, source_id: 9, external_downloader: true });
        assert_eq!(request_of(&[target(1), target(2)], &rows, false), None, "a multi-select opens nothing");
        assert_eq!(request_of(&[], &rows, false), None);
        assert_eq!(request_of(&[target(7)], &rows, false), None, "a row that left the feed");
    }

    #[test]
    fn the_sheet_is_reached_through_open() {
        assert!(!available());
        assert!(!open(QualityRequest { episode_title: String::new(), unit_id: 1, entry_id: 1, source_id: 1, external_downloader: false }));
        let seen = Rc::new(Cell::new(0));
        let counter = Rc::clone(&seen);
        install(move |request| counter.set(request.unit_id));
        assert!(available());
        assert!(open(QualityRequest { episode_title: String::new(), unit_id: 5, entry_id: 1, source_id: 1, external_downloader: false }));
        assert_eq!(seen.get(), 5);
    }

    #[test]
    fn the_episode_dialog_carries_the_episode_title() {
        let spec = crate::spec::Dialog { id: "episode_options".into(), title: "Episode 1".into(), text: Some("Choose video quality:".into()), ..Default::default() };
        let request = QualityRequest { episode_title: "Episode 9".into(), unit_id: 9, entry_id: 1, source_id: 2, external_downloader: false };
        let dialog = episode_dialog(&spec, &request);
        assert_eq!((dialog.title.as_str(), dialog.text.as_deref()), ("Episode 9", Some("Choose video quality:")));
    }
}
