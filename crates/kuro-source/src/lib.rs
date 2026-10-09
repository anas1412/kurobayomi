//! Source layer (areas 15 to 17). The source models, filters, traits and id derivation, plus the listing pager and the
//! error-message table. The source-ready gate lives in `kuro-core::sources`. No GTK and no OS code in this crate, and
//! no runtime is started here. Tokio gives the watch channels of the page state.

pub mod anime;
pub mod browse;
pub mod error;
pub mod factory;
pub mod filter;
pub mod filter_list;
pub mod filter_sheet;
pub mod id;
pub mod identity;
pub mod image_state;
pub mod json;
pub mod listing;
pub mod local;
pub mod messages;
pub mod model;
pub mod pager;
pub mod prefs;
pub mod source;

pub use error::SourceError;
pub use filter::{Filter, SortSelection, TriStateState};
pub use filter_list::FilterList;
pub use id::generate_id;
pub use identity::{display_name, name_for_manga_info};
pub use listing::{Listing, QUERY_LATEST, QUERY_POPULAR};
pub use messages::{SourceFailure, error_message, failure_message};
pub use pager::{Page, PagerError, RemotePager};
pub use prefs::{PrefDescriptorError, SourcePref, check_descriptors, preference_key};
pub use source::{
    CatalogueSource, ConfigurableSource, MangaSource, MangaUriType, ResolvableSource, ResolvedManga, isolate,
    resolve_manga_uri,
};
