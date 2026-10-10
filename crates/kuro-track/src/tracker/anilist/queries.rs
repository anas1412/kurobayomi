//! GraphQL documents, copied from `p1-data-and-config.md` section 4. Two deliberate differences: the add
//! mutations name their variable `$mediaId` (the variable list the task contract asks for), and the Viewer
//! query carries no variables.

pub const VIEWER: &str = "query User {
  Viewer {
    id
    mediaListOptions {
      scoreFormat
    }
  }
}";

pub const SEARCH_MANGA: &str = "query Search($query: String) {
  Page (perPage: 50) {
    media(search: $query, type: MANGA, format_not_in: [NOVEL]) {
      id
      staff {
        edges {
          role
          id
          node {
            name {
              full
              userPreferred
              native
            }
          }
        }
      }
      title {
        userPreferred
      }
      coverImage {
        large
      }
      format
      status
      chapters
      description
      startDate {
        year
        month
        day
      }
      averageScore
    }
  }
}";

pub const SEARCH_ANIME: &str = "query Search($query: String) {
  Page (perPage: 50) {
    media(search: $query, type: ANIME) {
      id
      studios {
        edges {
          isMain
          node {
            name
          }
        }
      }
      title {
        userPreferred
      }
      coverImage {
        large
      }
      format
      status
      episodes
      description
      startDate {
        year
        month
        day
      }
      averageScore
    }
  }
}";

pub const FIND_MANGA: &str = "query ($id: Int!, $manga_id: Int!) {
  Page {
    mediaList(userId: $id, type: MANGA, mediaId: $manga_id) {
      id
      status
      scoreRaw: score(format: POINT_100)
      progress
      private
      startedAt {
        year
        month
        day
      }
      completedAt {
        year
        month
        day
      }
      media {
        id
        title {
          userPreferred
        }
        coverImage {
          large
        }
        format
        status
        chapters
        description
        startDate {
          year
          month
          day
        }
        staff {
          edges {
            role
            id
            node {
              name {
                full
                userPreferred
                native
              }
            }
          }
        }
      }
    }
  }
}";

pub const FIND_ANIME: &str = "query ($id: Int!, $anime_id: Int!) {
  Page {
    mediaList(userId: $id, type: ANIME, mediaId: $anime_id) {
      id
      status
      scoreRaw: score(format: POINT_100)
      progress
      private
      startedAt {
        year
        month
        day
      }
      completedAt {
        year
        month
        day
      }
      media {
        id
        title {
          userPreferred
        }
        coverImage {
          large
        }
        format
        status
        episodes
        description
        startDate {
          year
          month
          day
        }
        studios {
          edges {
            isMain
            node {
              name
            }
          }
        }
      }
    }
  }
}";

pub const ADD_MANGA: &str = "mutation AddManga($mediaId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean) {
  SaveMediaListEntry (mediaId: $mediaId, progress: $progress, status: $status, private: $private) {
    id
    status
  }
}";

pub const ADD_ANIME: &str = "mutation AddAnime($mediaId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean) {
  SaveMediaListEntry (mediaId: $mediaId, progress: $progress, status: $status, private: $private) {
    id
    status
  }
}";

pub const UPDATE_MANGA: &str = "mutation UpdateManga(
  $listId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean,
  $score: Int, $startedAt: FuzzyDateInput, $completedAt: FuzzyDateInput
) {
  SaveMediaListEntry(
    id: $listId, progress: $progress, status: $status, private: $private,
    scoreRaw: $score, startedAt: $startedAt, completedAt: $completedAt
  ) {
    id
    status
    progress
  }
}";

pub const UPDATE_ANIME: &str = "mutation UpdateAnime(
  $listId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean,
  $score: Int, $startedAt: FuzzyDateInput, $completedAt: FuzzyDateInput
) {
  SaveMediaListEntry(
    id: $listId, progress: $progress, status: $status, private: $private,
    scoreRaw: $score, startedAt: $startedAt, completedAt: $completedAt
  ) {
    id
    status
    progress
  }
}";

/// Deletes one MediaList entry. `listId` is the MediaList id (`library_id`), never the local row id (E-20).
pub const DELETE_ENTRY: &str = "mutation DeleteMediaListEntry($listId: Int) {
  DeleteMediaListEntry(id: $listId) {
    deleted
  }
}";

/// The MAL id of an AniList media. Sent without a token (AniSkip lookup, E-43).
pub const MAL_ID: &str = "query ($id: Int) {
  Media(id: $id) {
    idMal
  }
}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_28a_016_manga_search_text_has_the_server_side_filters() {
        assert!(SEARCH_MANGA.contains("Page (perPage: 50)"));
        assert!(
            SEARCH_MANGA.contains("media(search: $query, type: MANGA, format_not_in: [NOVEL])")
        );
    }

    #[test]
    fn t_28a_017_anime_search_text_has_no_format_filter() {
        assert!(SEARCH_ANIME.contains("media(search: $query, type: ANIME)"));
        assert!(!SEARCH_ANIME.contains("format_not_in"));
    }

    #[test]
    fn t_28a_019_find_queries_use_the_default_page_and_point_100_score() {
        for q in [FIND_MANGA, FIND_ANIME] {
            assert!(q.contains("scoreRaw: score(format: POINT_100)"));
            assert!(q.contains("Page {"));
            assert!(!q.contains("perPage"));
        }
    }

    #[test]
    fn t_28a_020_add_mutations_declare_the_four_variables() {
        for q in [ADD_MANGA, ADD_ANIME] {
            assert!(q.contains("$mediaId: Int"));
            assert!(q.contains("$progress: Int"));
            assert!(q.contains("$status: MediaListStatus"));
            assert!(q.contains("$private: Boolean"));
        }
    }

    #[test]
    fn t_28a_022_delete_mutation_takes_the_media_list_id() {
        assert!(DELETE_ENTRY.contains("DeleteMediaListEntry(id: $listId)"));
        assert!(DELETE_ENTRY.contains("deleted"));
    }

    #[test]
    fn t_28a_034_mal_lookup_asks_for_id_mal_only() {
        assert!(MAL_ID.contains("Media(id: $id)"));
        assert!(MAL_ID.contains("idMal"));
    }
}
