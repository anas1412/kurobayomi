-- Updates feed window (manga schema). M and N are favourites added at 100 and 300; X is not a favourite.
-- The feed takes favourite chapters fetched after their entry was added and uploaded after the window start.
INSERT INTO mangas(_id, source, url, title, status, favorite, initialized, viewer, chapter_flags, cover_last_modified, date_added) VALUES
    (1, 7, '/m', 'M', 1, 1, 1, 0, 0, 0, 100),
    (2, 7, '/n', 'N', 1, 1, 1, 0, 0, 0, 300),
    (3, 7, '/x', 'X', 1, 0, 1, 0, 0, 0, 100);
INSERT INTO chapters(_id, manga_id, url, name, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) VALUES
    (11, 1, '/m/1', 'M 1', 0, 0, 0, 1, 1, 150, 90),
    (12, 1, '/m/2', 'M 2', 0, 0, 0, 2, 2, 160, 200),
    (21, 2, '/n/1', 'N 1', 0, 0, 0, 1, 1, 250, 280),
    (22, 2, '/n/2', 'N 2', 0, 0, 0, 2, 2, 400, 350),
    (31, 3, '/x/1', 'X 1', 0, 0, 0, 1, 1, 500, 450);
