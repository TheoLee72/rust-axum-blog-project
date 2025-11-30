-- Add up migration script here

ALTER TABLE post
ADD COLUMN title_ko TEXT,
ADD COLUMN content_ko TEXT,
ADD COLUMN summary_ko TEXT,
ADD COLUMN raw_text_ko TEXT,
-- NEW: Add thumbnail URL column
ADD COLUMN thumbnail_url TEXT NOT NULL DEFAULT 'https://theolee.net/static/uploads/test1.png';

UPDATE post
SET title_ko = title,
    content_ko = content,
    summary_ko = summary,
    raw_text_ko = raw_text;

ALTER TABLE post
ALTER COLUMN title_ko SET NOT NULL,
ALTER COLUMN content_ko SET NOT NULL,
ALTER COLUMN summary_ko SET NOT NULL,
ALTER COLUMN raw_text_ko SET NOT NULL;

ALTER TABLE post
ADD COLUMN content_tsv_ko tsvector
    GENERATED ALWAYS AS (to_tsvector('simple', COALESCE(raw_text_ko, ''))) STORED;



-- ============================================================================
-- Create indexes for search functionality
-- ============================================================================
-- GIN index for full-text search on content_tsv column
-- GIN (Generalized Inverted Index) is optimized for text search queries
-- Speeds up websearch_to_tsquery operations
CREATE INDEX post_tsv_gin_idx ON post USING gin (content_tsv);

-- HNSW index for vector similarity search on embedding column
-- HNSW (Hierarchical Navigable Small World) is optimized for vector operations
-- vector_cosine_ops uses cosine distance metric
-- Speeds up embedding <=> (distance operator) queries
CREATE INDEX post_embedding_hnsw_idx ON post USING hnsw (embedding vector_cosine_ops);

-- GIN index for full-text search on content_tsv column
-- NEW: GIN index for Korean full-text search
CREATE INDEX post_tsv_ko_gin_idx ON post USING gin (content_tsv_ko);

-- ============================================================================
-- Hybrid search function combining full-text and semantic search (BILINGUAL)
-- ============================================================================

-- Performs hybrid search using RRF (Reciprocal Rank Fusion) algorithm
-- Combines results from three sources: English Text, Korean Text, and Single Semantic Vector.
--
-- Parameters:
--   query_text: Search query string (e.g., "Rust web framework") - NOW SINGLE PARAMETER
--   query_embedding: 768-dimensional vector (used for both EN/KO semantic search) - OPTIONAL
--   match_count, offset_count, weights, rrf_k: Standard RRF parameters
CREATE OR REPLACE FUNCTION hybrid_search(
    query_text TEXT,
    query_embedding vector(768),
    match_count INT,    
    offset_count INT DEFAULT 0,
    full_text_weight FLOAT = 1,
    semantic_weight FLOAT = 1,
    rrf_k INT = 50
)
RETURNS SETOF post
LANGUAGE SQL
AS $$
-- 1. Full-text search results (English) - Runs if query_text is provided. Will only match English tokens.
WITH full_text_en AS (
    SELECT
        id,
        ROW_NUMBER() OVER (ORDER BY ts_rank_cd(content_tsv, websearch_to_tsquery(query_text)) DESC, id ASC) AS rank_ix
    FROM post
    -- The query_text is used for the English TSV
    WHERE query_text IS NOT NULL AND content_tsv @@ websearch_to_tsquery(query_text)
    LIMIT (match_count + offset_count) * 2
),
-- 2. Full-text search results (Korean) - Runs if query_text is provided. Will only match Korean tokens.
full_text_ko AS (
    SELECT
        id,
        -- Use 'simple' dictionary for Korean
        ROW_NUMBER() OVER (ORDER BY ts_rank_cd(content_tsv_ko, websearch_to_tsquery('simple', query_text)) DESC, id ASC) AS rank_ix
    FROM post
    -- The same query_text is used for the Korean TSV with the 'simple' dictionary
    WHERE query_text IS NOT NULL AND content_tsv_ko @@ websearch_to_tsquery('simple', query_text)
    LIMIT (match_count + offset_count) * 2
),
-- 3. Vector similarity search results (Semantic) - ONLY RUN IF query_embedding IS PROVIDED
semantic AS (
    SELECT
        id,
        ROW_NUMBER() OVER (ORDER BY embedding <=> query_embedding, id ASC) AS rank_ix
    FROM post
    WHERE query_embedding IS NOT NULL AND embedding <=> query_embedding < 0.8
    LIMIT (match_count + offset_count) * 2
),
-- Combine all unique IDs from all three searches
combined_results AS (
    SELECT id FROM full_text_en
    UNION
    SELECT id FROM full_text_ko
    UNION
    SELECT id FROM semantic
)
-- Combine results using RRF (Reciprocal Rank Fusion)
SELECT
    p.*
FROM
    combined_results cr
    LEFT JOIN full_text_en fte ON cr.id = fte.id
    LEFT JOIN full_text_ko ftk ON cr.id = ftk.id
    LEFT JOIN semantic s ON cr.id = s.id
    JOIN post p ON cr.id = p.id
ORDER BY
    -- RRF Score Calculation: Sum of RRF scores from all three potential searches
    (COALESCE(1.0 / (rrf_k + fte.rank_ix), 0.0) * full_text_weight) +
    (COALESCE(1.0 / (rrf_k + ftk.rank_ix), 0.0) * full_text_weight) +
    (COALESCE(1.0 / (rrf_k + s.rank_ix), 0.0) * semantic_weight)
    DESC,
    p.id ASC
LIMIT match_count
OFFSET offset_count
$$;

-- ============================================================================
-- Hybrid search count function for pagination metadata (BILINGUAL)
-- ============================================================================

-- Count total results matching hybrid search criteria
CREATE OR REPLACE FUNCTION hybrid_search_count(
    query_text TEXT DEFAULT NULL,                -- SIMPLIFIED: SINGLE TEXT INPUT
    query_embedding vector(768) DEFAULT NULL, 
    full_text_weight FLOAT = 1,
    semantic_weight FLOAT = 1,
    rrf_k INT = 50
)
RETURNS BIGINT
LANGUAGE SQL
AS $$
-- Full-text search matches (English)
WITH full_text_en AS (
    SELECT id
    FROM post
    WHERE query_text IS NOT NULL AND content_tsv @@ websearch_to_tsquery(query_text)
),
-- Full-text search matches (Korean)
full_text_ko AS (
    SELECT id
    FROM post
    WHERE query_text IS NOT NULL AND content_tsv_ko @@ websearch_to_tsquery('simple', query_text)
),
-- Vector similarity search matches (Semantic)
semantic AS (
    SELECT id
    FROM post
    WHERE query_embedding IS NOT NULL AND embedding <=> query_embedding < 0.8
)
-- Count unique posts from all three search methods
SELECT COUNT(DISTINCT id) AS total_count
FROM (
    SELECT id FROM full_text_en
    UNION
    SELECT id FROM full_text_ko
    UNION
    SELECT id FROM semantic
) AS combined_ids;
$$;