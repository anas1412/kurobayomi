//! Source models (`SManga`, `SChapter`, `Page`, `MangasPage`, `UpdateStrategy`), as the source API defines them.

pub mod chapter;
pub mod manga;
pub mod mangas_page;
pub mod page;
pub mod update_strategy;

pub use chapter::{SChapter, domain_chapter_number, normalize_scanlator};
pub use manga::{MangaStatus, SManga};
pub use mangas_page::MangasPage;
pub use page::{Page, PageState, PageStatus};
pub use update_strategy::UpdateStrategy;
