-- Drop the functions that were created/replaced, as they relied on the new columns.
DROP FUNCTION IF EXISTS hybrid_search(TEXT, vector, INT, INT, FLOAT, FLOAT, INT);
DROP FUNCTION IF EXISTS hybrid_search_count(TEXT, vector, FLOAT, FLOAT, INT);

-- Drop the newly created index for the Korean TSV column.
DROP INDEX IF EXISTS post_tsv_ko_gin_idx;
DROP INDEX IF EXISTS post_tsv_gin_idx;
DROP INDEX IF EXISTS post_embedding_hnsw_idx;

-- Alter the post table to drop all added columns.
ALTER TABLE post
    DROP COLUMN title_ko,
    DROP COLUMN content_ko,
    DROP COLUMN summary_ko,
    DROP COLUMN content_tsv_ko, -- Generated columns are dropped just like regular columns
    DROP COLUMN raw_text_ko,
    DROP COLUMN thumbnail_url;