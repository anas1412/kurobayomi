-- Two manga (manga schema): M with chapters 1 and 2, N with chapters 3 and 4. Four history rows, one per chapter.
-- History row 4 has never been read (timestamp 0), so it is not listed.
INSERT INTO mangas(_id, source, url, title, status, favorite, initialized, viewer, chapter_flags, cover_last_modified, date_added) VALUES
    (1, 7, '/m', 'M', 1, 1, 1, 0, 0, 0, 1),
    (2, 7, '/n', 'N', 1, 1, 1, 0, 0, 0, 1);
INSERT INTO chapters(_id, manga_id, url, name, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) VALUES
    (1, 1, '/m/1', 'M c1', 1, 0, 0, 1, 1, 0, 0),
    (2, 1, '/m/2', 'M c2', 1, 0, 0, 2, 2, 0, 0),
    (3, 2, '/n/1', 'N n1', 0, 0, 0, 1, 1, 0, 0),
    (4, 2, '/n/2', 'N n2', 0, 0, 0, 2, 2, 0, 0);
INSERT INTO history(_id, chapter_id, last_read, time_read) VALUES
    (1, 1, 1000, 500),
    (2, 2, 2000, 700),
    (3, 3, 1500, 300),
    (4, 4, 0, 0);
