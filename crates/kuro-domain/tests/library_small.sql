-- Small library (manga schema). Two user categories, the second hidden; three favourites; one entry that is not in
-- the library. Timestamps are small integers, as in golden/sort_orders.json.
INSERT INTO categories(_id, name, sort, flags, hidden) VALUES
    (1, 'Reading', 1, 0, 0),
    (2, 'Hold', 2, 0, 1);
INSERT INTO mangas(_id, source, url, title, status, favorite, initialized, viewer, chapter_flags, cover_last_modified, date_added) VALUES
    (1, 7, '/alpha', 'Alpha', 1, 1, 1, 0, 0, 0, 1),
    (2, 7, '/beta', 'beta', 1, 1, 1, 0, 0, 0, 2),
    (3, 7, '/gamma', 'Gamma', 2, 1, 1, 0, 0, 0, 3),
    (4, 7, '/emile', 'Émile', 1, 0, 1, 0, 0, 0, 4);
INSERT INTO chapters(_id, manga_id, url, name, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) VALUES
    (11, 1, '/alpha/1', 'Alpha 1', 1, 0, 0, 1, 1, 10, 0),
    (12, 1, '/alpha/2', 'Alpha 2', 1, 0, 0, 2, 2, 10, 0),
    (21, 2, '/beta/1', 'beta 1', 1, 0, 0, 1, 1, 20, 0),
    (22, 2, '/beta/2', 'beta 2', 0, 0, 0, 2, 2, 20, 0),
    (31, 3, '/gamma/1', 'Gamma 1', 0, 0, 0, 1, 1, 30, 0),
    (41, 4, '/emile/1', 'Émile 1', 1, 0, 0, 1, 1, 40, 0);
INSERT INTO mangas_categories(manga_id, category_id) VALUES
    (1, 1),
    (2, 1),
    (2, 2);
