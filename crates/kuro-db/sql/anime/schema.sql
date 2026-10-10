-- Kurobayomi anime database: fresh schema at user_version 140 (the Aniyomi schema 139 plus native step 139 to 140).
-- Source: Aniyomi data/src/main/sqldelightanime/dataanime/*.sq. SQLDelight annotations
-- are removed and double-quoted strings are written as single-quoted literals.
-- Views are in views.sql.

CREATE TABLE animes(
    _id INTEGER NOT NULL PRIMARY KEY,
    source INTEGER NOT NULL,
    url TEXT NOT NULL,
    artist TEXT,
    author TEXT,
    description TEXT,
    genre TEXT,
    title TEXT NOT NULL,
    status INTEGER NOT NULL,
    thumbnail_url TEXT,
    favorite INTEGER NOT NULL,
    last_update INTEGER,
    next_update INTEGER,
    initialized INTEGER NOT NULL,
    viewer INTEGER NOT NULL,
    episode_flags INTEGER NOT NULL,
    cover_last_modified INTEGER NOT NULL,
    date_added INTEGER NOT NULL,
    update_strategy INTEGER NOT NULL DEFAULT 0,
    calculate_interval INTEGER DEFAULT 0 NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    favorite_modified_at INTEGER,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    fetch_type INTEGER NOT NULL DEFAULT 1,
    parent_id INTEGER,
    season_flags INTEGER NOT NULL,
    season_number REAL NOT NULL,
    season_source_order INTEGER NOT NULL,
    background_url TEXT,
    background_last_modified INTEGER NOT NULL,
    memo BLOB NOT NULL DEFAULT '{}'
);

CREATE INDEX animelib_favorite_index ON animes(favorite) WHERE favorite = 1;

CREATE INDEX animes_url_index ON animes(url);

CREATE INDEX animes_parent_id ON animes(parent_id);

CREATE INDEX animes_fetch_type ON animes(fetch_type);

CREATE TRIGGER update_last_favorite_at_animes
AFTER UPDATE OF favorite ON animes
BEGIN
  UPDATE animes
  SET favorite_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;

CREATE TRIGGER update_last_modified_at_animes
AFTER UPDATE ON animes
FOR EACH ROW
BEGIN
  UPDATE animes
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;

CREATE TRIGGER update_anime_version AFTER UPDATE ON animes
BEGIN
    UPDATE animes SET version = version + 1
    WHERE _id = new._id AND new.is_syncing = 0 AND (
        new.url != old.url OR
        new.description != old.description OR
        new.favorite != old.favorite
    );
END;

CREATE TABLE episodes(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT,
    seen INTEGER NOT NULL,
    bookmark INTEGER NOT NULL,
    last_second_seen INTEGER NOT NULL,
    total_seconds INTEGER NOT NULL,
    episode_number REAL NOT NULL,
    source_order INTEGER NOT NULL,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    summary TEXT,
    preview_url TEXT,
    fillermark INTEGER NOT NULL,
    memo BLOB NOT NULL DEFAULT '{}',
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);

CREATE INDEX episodes_anime_id_index ON episodes(anime_id);

CREATE INDEX episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0;

CREATE TRIGGER update_last_modified_at_episodes
AFTER UPDATE ON episodes
FOR EACH ROW
BEGIN
  UPDATE episodes
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;

CREATE TRIGGER update_episode_and_anime_version AFTER UPDATE ON episodes
WHEN new.is_syncing = 0 AND (
    new.seen != old.seen OR
    new.bookmark != old.bookmark OR
    new.last_second_seen != old.last_second_seen
)
BEGIN
    -- Update the episode version
    UPDATE episodes SET version = version + 1
    WHERE _id = new._id;

    -- Update the anime version
    UPDATE animes SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;

CREATE TABLE animehistory(
    _id INTEGER NOT NULL PRIMARY KEY,
    episode_id INTEGER NOT NULL UNIQUE,
    last_seen INTEGER,
    FOREIGN KEY(episode_id) REFERENCES episodes (_id)
    ON DELETE CASCADE
);

CREATE INDEX animehistory_history_episode_id_index ON animehistory(episode_id);

CREATE TABLE animes_categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    FOREIGN KEY(category_id) REFERENCES categories (_id)
    ON DELETE CASCADE,
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);

CREATE INDEX animes_categories_anime_id_index ON animes_categories(anime_id);

CREATE INDEX animes_categories_category_id_index ON animes_categories(category_id);

CREATE TRIGGER insert_anime_category_update_version AFTER INSERT ON animes_categories
BEGIN
    UPDATE animes
    SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;

CREATE TABLE categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    sort INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0
);

-- Insert system category
INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, '', -1, 64);

-- Disallow deletion of default category
CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE
ON categories
BEGIN SELECT CASE
    WHEN old._id <= 0 THEN
        RAISE(ABORT, 'System category can''t be deleted')
    END;
END;

CREATE TABLE anime_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_episode_seen REAL NOT NULL,
    total_episodes INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER DEFAULT 0 NOT NULL,
    UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);

CREATE TABLE animesources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE anime_relations(
    anime_id INTEGER NOT NULL,
    related_anime_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    last_fetched_at INTEGER NOT NULL,
    PRIMARY KEY(anime_id, related_anime_id, name),
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE,
    FOREIGN KEY(related_anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);

CREATE INDEX anime_relations_anime_id_index ON anime_relations(anime_id);

CREATE INDEX anime_relations_related_anime_id_index ON anime_relations(related_anime_id);

CREATE TABLE custom_buttons (
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    isFavorite INTEGER NOT NULL,
    sortIndex INTEGER NOT NULL,
    content TEXT NOT NULL,
    longPressContent TEXT NOT NULL,
    onStartup TEXT NOT NULL
);

-- Insert skip intro button --
INSERT OR IGNORE INTO custom_buttons (_id, name, isFavorite, sortIndex, content, longPressContent, onStartup)
VALUES (1, '+85 s', 1, 0, 'local intro_length = mp.get_property_number("user-data/current-anime/intro-length")
aniyomi.right_seek_by(intro_length)', 'aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")', 'function update_button(_, length)
  if length ~= nil then
    if length == 0 then
	  aniyomi.hide_button()
	  return
	else
	  aniyomi.show_button()
	end
    aniyomi.set_button_title("+" .. length .. " s")
  end
end

if $isPrimary then
  mp.observe_property("user-data/current-anime/intro-length", "number", update_button)
end');

CREATE TABLE extension_store(
    index_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    badge_label TEXT NOT NULL,
    signing_key TEXT NOT NULL,
    contact_website TEXT NOT NULL,
    contact_discord TEXT,
    is_legacy INTEGER NOT NULL,
    extension_list_url TEXT,
    resolved_index_url TEXT NOT NULL
);
