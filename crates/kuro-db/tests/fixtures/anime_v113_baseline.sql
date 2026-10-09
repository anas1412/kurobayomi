-- The reconstructed version-113 anime database (p1-topic-legacy-migration-chain.md section 2) with the seed rows of
-- p1-edge-cases-and-tests.md section 7. Run with foreign_keys OFF, then set PRAGMA user_version = 113.
CREATE TABLE animes(_id INTEGER NOT NULL PRIMARY KEY, source INTEGER NOT NULL, url TEXT NOT NULL, artist TEXT, author TEXT, description TEXT, genre TEXT, title TEXT NOT NULL, status INTEGER NOT NULL, thumbnail_url TEXT, favorite INTEGER NOT NULL, last_update INTEGER, next_update INTEGER, initialized INTEGER NOT NULL, viewer INTEGER NOT NULL, episode_flags INTEGER NOT NULL, cover_last_modified INTEGER NOT NULL, date_added INTEGER NOT NULL);
CREATE TABLE categories(_id INTEGER NOT NULL PRIMARY KEY, name TEXT NOT NULL, sort INTEGER NOT NULL, flags INTEGER NOT NULL);
CREATE TABLE episodes(_id INTEGER NOT NULL PRIMARY KEY, anime_id INTEGER NOT NULL, url TEXT NOT NULL, name TEXT NOT NULL, scanlator TEXT, seen INTEGER NOT NULL, bookmark INTEGER NOT NULL, last_second_seen INTEGER NOT NULL, total_seconds INTEGER NOT NULL, episode_number REAL NOT NULL, source_order INTEGER NOT NULL, date_fetch INTEGER NOT NULL, date_upload INTEGER NOT NULL, FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE);
CREATE TABLE animehistory(animehistory_id INTEGER NOT NULL PRIMARY KEY, animehistory_episode_id INTEGER NOT NULL UNIQUE, animehistory_last_seen INTEGER, animehistory_time_seen INTEGER, FOREIGN KEY(animehistory_episode_id) REFERENCES episodes (_id) ON DELETE CASCADE);
CREATE TABLE animes_categories(_id INTEGER NOT NULL PRIMARY KEY, anime_id INTEGER NOT NULL, category_id INTEGER NOT NULL, FOREIGN KEY(category_id) REFERENCES categories (_id) ON DELETE CASCADE, FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE);
CREATE TABLE anime_sync(_id INTEGER NOT NULL PRIMARY KEY, anime_id INTEGER NOT NULL, sync_id INTEGER NOT NULL, remote_id INTEGER NOT NULL, library_id INTEGER, title TEXT NOT NULL, last_episode_seen REAL NOT NULL, total_episodes INTEGER NOT NULL, status INTEGER NOT NULL, score REAL NOT NULL, remote_url TEXT NOT NULL, start_date INTEGER NOT NULL, finish_date INTEGER NOT NULL, UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE, FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE);
CREATE INDEX episodes_anime_id_index ON episodes(anime_id);
CREATE INDEX episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0;
CREATE INDEX animehistory_history_episode_id_index ON animehistory(animehistory_episode_id);
CREATE INDEX library_favorite_index ON animes(favorite) WHERE favorite = 1;
CREATE INDEX animes_url_index ON animes(url);

INSERT INTO categories VALUES (1,'cat',0,64);
INSERT INTO animes VALUES (1,100,'/a',NULL,NULL,NULL,'Action, Drama','Alpha',1,NULL,1,5,6,1,0,0,0,1000);
INSERT INTO animes VALUES (2,100,'/b',NULL,NULL,NULL,NULL,'Beta',2,NULL,0,NULL,NULL,0,0,0,0,0);
INSERT INTO episodes VALUES (1,1,'/e1','E1',NULL,1,0,0,1400,1.0,0,50,0);
INSERT INTO episodes VALUES (2,1,'/e2','E2',NULL,0,1,300,1400,2.0,1,60,0);
INSERT INTO animehistory VALUES (1,1,NULL,500);
INSERT INTO animehistory VALUES (2,2,9000,500);
INSERT INTO animes_categories VALUES (1,1,1);
INSERT INTO anime_sync VALUES (1,1,2,77,NULL,'t',1.0,12,1,8.0,'u',0,0);
