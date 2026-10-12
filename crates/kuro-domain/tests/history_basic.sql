-- Two manga (manga schema): M with chapters 1 and 2, N with chapters 3 and 4. Four history rows, one per chapter.
-- History row 4 has never been read (timestamp 0), so it is not listed.
INSERT INTO entries(_id, kind, source, url, title, status, favorite, initialized, viewer, item_flags, cover_last_modified, date_added) VALUES
    (1, 'manga', 7, '/m', 'M', 1, 1, 1, 0, 0, 0, 1),
    (2, 'manga', 7, '/n', 'N', 1, 1, 1, 0, 0, 0, 1);
INSERT INTO items(_id, entry_id, url, name, done, bookmark, progress, number, source_order, date_fetch, date_upload) VALUES
    (1, 1, '/m/1', 'M c1', 1, 0, 0, 1, 1, 0, 0),
    (2, 1, '/m/2', 'M c2', 1, 0, 0, 2, 2, 0, 0),
    (3, 2, '/n/1', 'N n1', 0, 0, 0, 1, 1, 0, 0),
    (4, 2, '/n/2', 'N n2', 0, 0, 0, 2, 2, 0, 0);
INSERT INTO history(_id, item_id, last_at, duration_ms) VALUES
    (1, 1, 1000, 500),
    (2, 2, 2000, 700),
    (3, 3, 1500, 300),
    (4, 4, 0, 0);
