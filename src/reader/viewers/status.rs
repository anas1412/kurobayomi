//! Page status observation (T-44a-006, B§8.1, E-44). Only the latest status branch runs: a progress update that
//! arrives after the page is ready (or after an error) is dropped, as `collectLatest` cancels the progress branch.

use crate::reader::model::PageStatus;

/// What a page's holder shows for a status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageView {
    /// Queued or loading: the spinner.
    Spinner,
    /// Downloading: the determinate arc at 0 to 100.
    Progress(i32),
    /// The image is ready to decode and show.
    Ready,
    /// The error card with retry.
    Error,
}

/// The status branch of one page holder.
#[derive(Clone, Debug, Default)]
pub struct StatusBranch {
    status: Option<PageStatus>,
}

impl StatusBranch {
    /// A new status replaces the branch and says what the holder shows.
    pub fn on_status(&mut self, status: PageStatus) -> PageView {
        self.status = Some(status);
        match status {
            PageStatus::Queued | PageStatus::LoadPage => PageView::Spinner,
            PageStatus::DownloadImage => PageView::Progress(0),
            PageStatus::Ready => PageView::Ready,
            PageStatus::Error => PageView::Error,
        }
    }

    /// A progress update (0 to 100). It applies only while the download branch is the latest one.
    pub fn on_progress(&self, value: i32) -> Option<PageView> {
        (self.status == Some(PageStatus::DownloadImage)).then(|| PageView::Progress(value.clamp(0, 100)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_map_to_the_holder_views() {
        let mut b = StatusBranch::default();
        assert_eq!(b.on_status(PageStatus::Queued), PageView::Spinner);
        assert_eq!(b.on_status(PageStatus::LoadPage), PageView::Spinner);
        assert_eq!(b.on_status(PageStatus::DownloadImage), PageView::Progress(0));
        assert_eq!(b.on_status(PageStatus::Ready), PageView::Ready);
        assert_eq!(b.on_status(PageStatus::Error), PageView::Error);
    }

    #[test]
    fn progress_updates_apply_only_while_downloading() {
        let mut b = StatusBranch::default();
        assert_eq!(b.on_progress(40), None);
        b.on_status(PageStatus::DownloadImage);
        assert_eq!(b.on_progress(40), Some(PageView::Progress(40)));
        assert_eq!(b.on_progress(250), Some(PageView::Progress(100)));
        b.on_status(PageStatus::Ready);
        assert_eq!(b.on_progress(60), None);
    }
}
