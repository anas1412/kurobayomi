
-- Add private field for tracking
ALTER TABLE manga_sync ADD COLUMN private INTEGER  DEFAULT 0 NOT NULL;
