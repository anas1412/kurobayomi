-- Small library (manga schema). Two user categories, the second hidden; three favourites; one entry that is not in
-- the library. Timestamps are small integers, as in golden/sort_orders.json.
INSERT INTO categories(kind, _id, name, sort, flags, hidden) VALUES
    ('manga', 1, 'Reading', 1, 0, 0),
    ('manga', 2, 'Hold', 2, 0, 1);
INSERT INTO entries(_id, kind, source, url, title, status, favorite, initialized, viewer, item_flags, cover_last_modified, date_added) VALUES
    (1, 'manga', 7, '/alpha', 'Alpha', 1, 1, 1, 0, 0, 0, 1),
    (2, 'manga', 7, '/beta', 'beta', 1, 1, 1, 0, 0, 0, 2),
    (3, 'manga', 7, '/gamma', 'Gamma', 2, 1, 1, 0, 0, 0, 3),
    (4, 'manga', 7, '/emile', 'Émile', 1, 0, 1, 0, 0, 0, 4);
INSERT INTO items(_id, entry_id, url, name, done, bookmark, progress, number, source_order, date_fetch, date_upload) VALUES
    (11, 1, '/alpha/1', 'Alpha 1', 1, 0, 0, 1, 1, 10, 0),
    (12, 1, '/alpha/2', 'Alpha 2', 1, 0, 0, 2, 2, 10, 0),
    (21, 2, '/beta/1', 'beta 1', 1, 0, 0, 1, 1, 20, 0),
    (22, 2, '/beta/2', 'beta 2', 0, 0, 0, 2, 2, 20, 0),
    (31, 3, '/gamma/1', 'Gamma 1', 0, 0, 0, 1, 1, 30, 0),
    (41, 4, '/emile/1', 'Émile 1', 1, 0, 0, 1, 1, 40, 0);
INSERT INTO entry_categories(entry_id, kind, category_id) VALUES
    (1, 'manga', 1),
    (2, 'manga', 1),
    (2, 'manga', 2);
