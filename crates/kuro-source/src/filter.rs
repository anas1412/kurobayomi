//! Browse filters (`Filter.kt`, T-15a-008, T-15a-009, T-15a-010). A source defines them; the UI edits their state.

use crate::error::SourceError;

/// A tri-state checkbox. The raw values are 0, 1 and 2 (`TriState.kt`). Serde writes the raw value, not the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub enum TriStateState {
    #[default]
    Ignore = 0,
    Include = 1,
    Exclude = 2,
}

impl From<TriStateState> for i32 {
    fn from(state: TriStateState) -> Self {
        state.raw()
    }
}

impl TryFrom<i32> for TriStateState {
    type Error = SourceError;

    fn try_from(raw: i32) -> Result<Self, Self::Error> {
        Self::from_raw(raw)
    }
}

impl TriStateState {
    /// Maps a raw value to a state. Any other value (for example 3 or -1) is a parse error (EC-FI-02).
    pub fn from_raw(raw: i32) -> Result<Self, SourceError> {
        match raw {
            0 => Ok(Self::Ignore),
            1 => Ok(Self::Include),
            2 => Ok(Self::Exclude),
            other => Err(SourceError::Parse(format!("unknown tri-state value {other}"))),
        }
    }

    pub fn raw(self) -> i32 {
        self as i32
    }

    /// The state after one tap: `Ignore`, then `Include`, then `Exclude`, then `Ignore` again.
    pub fn next(self) -> Self {
        match self {
            Self::Ignore => Self::Include,
            Self::Include => Self::Exclude,
            Self::Exclude => Self::Ignore,
        }
    }
}

/// The chosen sort column and direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SortSelection {
    pub index: i32,
    pub ascending: bool,
}

impl SortSelection {
    /// The selection after a tap on row `index` (EC-FI-03..05). The same row flips the direction. Another row
    /// takes the selection and keeps the direction. With no selection, the tapped row is ascending.
    pub fn tap(current: Option<SortSelection>, index: i32) -> SortSelection {
        match current {
            None => SortSelection { index, ascending: true },
            Some(sel) if sel.index == index => SortSelection { index, ascending: !sel.ascending },
            Some(sel) => SortSelection { index, ascending: sel.ascending },
        }
    }
}

/// One filter of a source. Equality is the name and the state, as Kotlin's `Filter.equals`; the variant, the values and
/// the other fields are not compared. Equal filters hash the same.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Filter {
    Header {
        name: String,
    },
    Separator {
        name: String,
    },
    /// `state` is an index into `values`. It is not bounds-checked; read it through [`Filter::select_value`].
    Select {
        name: String,
        values: Vec<String>,
        state: i32,
    },
    Text {
        name: String,
        state: String,
    },
    CheckBox {
        name: String,
        state: bool,
    },
    TriState {
        name: String,
        state: TriStateState,
    },
    Group {
        name: String,
        state: Vec<Filter>,
    },
    /// `None` means no explicit sort; the source then applies its own default.
    Sort {
        name: String,
        values: Vec<String>,
        state: Option<SortSelection>,
    },
}

/// The state that equality and hashing read. Kotlin keeps a header, a separator, a select and a tri-state as the same
/// boxed integer (`Header` and `Separator` hold 0), so those four share `Int`.
#[derive(PartialEq, Eq, Hash)]
enum FilterState<'a> {
    Int(i32),
    Text(&'a str),
    CheckBox(bool),
    Group(&'a [Filter]),
    Sort(Option<SortSelection>),
}

impl Filter {
    fn identity(&self) -> (&str, FilterState<'_>) {
        match self {
            Self::Header { name } | Self::Separator { name } => (name, FilterState::Int(0)),
            Self::Select { name, state, .. } => (name, FilterState::Int(*state)),
            Self::Text { name, state } => (name, FilterState::Text(state)),
            Self::CheckBox { name, state } => (name, FilterState::CheckBox(*state)),
            Self::TriState { name, state } => (name, FilterState::Int(state.raw())),
            Self::Group { name, state } => (name, FilterState::Group(state)),
            Self::Sort { name, state, .. } => (name, FilterState::Sort(*state)),
        }
    }
}

impl PartialEq for Filter {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for Filter {}

impl std::hash::Hash for Filter {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.identity().hash(hasher);
    }
}

impl Filter {
    pub fn header(name: impl Into<String>) -> Self {
        Self::Header { name: name.into() }
    }

    pub fn separator(name: impl Into<String>) -> Self {
        Self::Separator { name: name.into() }
    }

    /// A select filter on index 0.
    pub fn select(name: impl Into<String>, values: Vec<String>) -> Self {
        Self::Select { name: name.into(), values, state: 0 }
    }

    /// A select filter on `index`. An index outside `values` falls back to 0 with a warning (E-14); empty values
    /// always give 0 and no value (`select_value` is `None`).
    pub fn select_at(name: impl Into<String>, values: Vec<String>, index: i32) -> Self {
        let in_range = usize::try_from(index).is_ok_and(|i| i < values.len());
        if !in_range && !values.is_empty() {
            tracing::warn!(index, len = values.len(), "select index out of range, using 0");
        }
        let state = if in_range { index } else { 0 };
        Self::Select { name: name.into(), values, state }
    }

    /// An empty text filter.
    pub fn text(name: impl Into<String>) -> Self {
        Self::Text { name: name.into(), state: String::new() }
    }

    /// An unchecked checkbox.
    pub fn check_box(name: impl Into<String>) -> Self {
        Self::CheckBox { name: name.into(), state: false }
    }

    /// A tri-state filter in `Ignore`.
    pub fn tri_state(name: impl Into<String>) -> Self {
        Self::TriState { name: name.into(), state: TriStateState::Ignore }
    }

    pub fn group(name: impl Into<String>, state: Vec<Filter>) -> Self {
        Self::Group { name: name.into(), state }
    }

    /// A sort filter with no selection.
    pub fn sort(name: impl Into<String>, values: Vec<String>) -> Self {
        Self::Sort { name: name.into(), values, state: None }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Header { name }
            | Self::Separator { name }
            | Self::Select { name, .. }
            | Self::Text { name, .. }
            | Self::CheckBox { name, .. }
            | Self::TriState { name, .. }
            | Self::Group { name, .. }
            | Self::Sort { name, .. } => name,
        }
    }

    /// The chosen value of a select filter, or `None` when the index is out of range (EC-FI-01) or the filter is
    /// not a select.
    pub fn select_value(&self) -> Option<&str> {
        match self {
            Self::Select { values, state, .. } => {
                let index = usize::try_from(*state).ok()?;
                values.get(index).map(String::as_str)
            }
            _ => None,
        }
    }

    /// This filter and every filter nested in it, depth first (EC-FI-07).
    pub fn walk(&self) -> Vec<&Filter> {
        let mut out = vec![self];
        if let Self::Group { state, .. } = self {
            for nested in state {
                out.extend(nested.walk());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::*;

    fn hash_of(f: &Filter) -> u64 {
        let mut h = DefaultHasher::new();
        f.hash(&mut h);
        h.finish()
    }

    fn values() -> Vec<String> {
        ["A", "B", "C"].map(str::to_owned).to_vec()
    }

    #[test]
    fn every_variant_round_trips_through_json_to_the_same_json() {
        let filters = vec![
            Filter::header("H"),
            Filter::separator("S"),
            Filter::select_at("Select", values(), 1),
            Filter::Text { name: "Text".into(), state: "query".into() },
            Filter::check_box("Check"),
            Filter::TriState { name: "Tri".into(), state: TriStateState::Exclude },
            Filter::group("Group", vec![Filter::tri_state("inner")]),
            Filter::Sort {
                name: "Sort".into(),
                values: values(),
                state: Some(SortSelection { index: 2, ascending: false }),
            },
        ];
        let json = serde_json::to_string(&filters).expect("serialise");
        let back: Vec<Filter> = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, filters);
        assert_eq!(serde_json::to_string(&back).expect("serialise"), json);
        assert!(json.contains(r#""TriState":{"name":"Tri","state":2}"#), "the tri-state is its raw value: {json}");
        assert!(serde_json::from_str::<TriStateState>("3").is_err(), "an unknown raw value is rejected");
    }

    #[test]
    fn defaults_match_the_kotlin_states() {
        assert_eq!(
            Filter::select("Pick", values()),
            Filter::Select { name: "Pick".into(), values: values(), state: 0 }
        );
        assert_eq!(Filter::text("Query"), Filter::Text { name: "Query".into(), state: String::new() });
        assert_eq!(Filter::check_box("Flag"), Filter::CheckBox { name: "Flag".into(), state: false });
        assert_eq!(Filter::header("H"), Filter::Header { name: "H".into() });
        assert_eq!(Filter::separator("S"), Filter::Separator { name: "S".into() });
    }

    #[test]
    fn equality_ignores_values_and_text() {
        let short = Filter::select_at("Order", vec!["A".into()], 0);
        let long = Filter::select_at("Order", vec!["A".into(), "B".into()], 0);
        assert_eq!(short, long, "Kotlin Filter.equals compares the name and the state only");
        assert_eq!(Filter::header("Same"), Filter::separator("Same"), "Kotlin: both hold state 0");
        assert_ne!(Filter::select_at("Order", values(), 0), Filter::select_at("Order", values(), 1));
    }

    #[test]
    fn equality_is_name_plus_state_and_hash_agrees() {
        let a = Filter::check_box("Flag");
        let mut b = Filter::check_box("Flag");
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        if let Filter::CheckBox { state, .. } = &mut b {
            *state = true;
        }
        assert_ne!(a, b);
        assert_ne!(Filter::check_box("Other"), Filter::check_box("Flag"));
    }

    #[test]
    fn select_at_clamps_an_out_of_range_index_to_zero() {
        assert_eq!(Filter::select_at("Pick", values(), 2).select_value(), Some("C"));
        assert_eq!(
            Filter::select_at("Pick", values(), 5),
            Filter::Select { name: "Pick".into(), values: values(), state: 0 }
        );
        assert_eq!(Filter::select_at("Pick", values(), -1).select_value(), Some("A"));
        assert_eq!(
            Filter::select_at("Pick", Vec::new(), 3),
            Filter::Select { name: "Pick".into(), values: Vec::new(), state: 0 }
        );
    }

    #[test]
    fn select_value_is_none_out_of_range() {
        let mut f = Filter::select("Pick", values());
        assert_eq!(f.select_value(), Some("A"));
        if let Filter::Select { state, .. } = &mut f {
            *state = 5;
        }
        assert_eq!(f.select_value(), None);
        if let Filter::Select { state, .. } = &mut f {
            *state = -1;
        }
        assert_eq!(f.select_value(), None);
        assert_eq!(Filter::text("x").select_value(), None);
    }

    #[test]
    fn tri_state_raw_values_map_and_others_are_parse_errors() {
        assert_eq!(TriStateState::from_raw(0).unwrap(), TriStateState::Ignore);
        assert_eq!(TriStateState::from_raw(1).unwrap(), TriStateState::Include);
        assert_eq!(TriStateState::from_raw(2).unwrap(), TriStateState::Exclude);
        for bad in [3, -1] {
            assert!(matches!(TriStateState::from_raw(bad), Err(SourceError::Parse(_))));
        }
        assert_eq!(TriStateState::Exclude.raw(), 2);
    }

    #[test]
    fn tri_state_cycles_ignore_include_exclude() {
        let mut s = TriStateState::Ignore;
        let cycle: Vec<_> = (0..4)
            .map(|_| {
                let now = s;
                s = s.next();
                now
            })
            .collect();
        assert_eq!(
            cycle,
            vec![TriStateState::Ignore, TriStateState::Include, TriStateState::Exclude, TriStateState::Ignore]
        );
    }

    #[test]
    fn tapping_the_selected_sort_row_flips_and_another_row_keeps_direction() {
        let sel = |index, ascending| Some(SortSelection { index, ascending });
        assert_eq!(SortSelection::tap(sel(1, true), 1), SortSelection { index: 1, ascending: false });
        assert_eq!(SortSelection::tap(sel(1, false), 0), SortSelection { index: 0, ascending: false });
    }

    #[test]
    fn tapping_with_no_sort_selected_picks_the_row_ascending() {
        assert_eq!(SortSelection::tap(None, 2), SortSelection { index: 2, ascending: true });
    }

    #[test]
    fn walk_finds_filters_at_every_depth() {
        let deep = Filter::tri_state("Deep");
        let inner = Filter::group("Inner", vec![deep.clone()]);
        let outer = Filter::group("Outer", vec![Filter::text("Q"), inner]);
        let names: Vec<&str> = outer.walk().iter().map(|f| f.name()).collect();
        assert_eq!(names, vec!["Outer", "Q", "Inner", "Deep"]);
    }
}
