-- Add up migration script here

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

-- ============================================================================
-- Hybrid search function combining full-text and semantic search
-- ============================================================================

-- Performs hybrid search using RRF (Reciprocal Rank Fusion) algorithm
-- Combines results from full-text search and vector similarity search
-- Ranks results by weighted combination of both methods
--
-- Parameters:
--   query_text: Search query string (e.g., "Rust web framework")
--   query_embedding: 768-dimensional vector from embedding service
--   match_count: Number of results to return (LIMIT)
--   offset_count: Number of results to skip (OFFSET) for pagination
--   full_text_weight: Weight for full-text search ranking (default 1.0)
--   semantic_weight: Weight for semantic/vector search ranking (default 1.0)
--   rrf_k: RRF constant parameter affecting rank normalization (default 50, higher = more balanced)
--
-- RRF Formula: 1 / (k + rank) - converts ranks to normalized scores
-- Higher k value gives more balanced weighting between methods
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
-- Full-text search results with ranking
WITH full_text AS (
  SELECT
    id,
    -- Rank by tsvector match quality (ts_rank_cd), then by ID for consistency
    ROW_NUMBER() OVER (ORDER BY ts_rank_cd(content_tsv, websearch_to_tsquery(query_text)) DESC, id ASC) AS rank_ix
  FROM
    post
  WHERE
    -- Match query against tsvector column using websearch syntax
    content_tsv @@ websearch_to_tsquery(query_text)
  LIMIT (match_count + offset_count) * 2
),
-- Vector similarity search results with ranking
semantic AS (
  SELECT
    id,
    -- Rank by vector distance (<=> cosine distance operator), then by ID
    ROW_NUMBER() OVER (ORDER BY embedding <=> query_embedding, id ASC) AS rank_ix
  FROM
    post
  WHERE
    -- Only include results within reasonable similarity threshold (< 0.8 distance)
    embedding <=> query_embedding < 0.8
  LIMIT (match_count + offset_count) * 2
)
-- Combine results using RRF (Reciprocal Rank Fusion)
SELECT
  post.*
FROM
  full_text
  -- FULL OUTER JOIN includes results from either search method
  FULL OUTER JOIN semantic ON full_text.id = semantic.id
  JOIN post ON COALESCE(full_text.id, semantic.id) = post.id
ORDER BY
  -- RRF score calculation: 1/(k+rank) weighted by importance
  -- COALESCE returns 0 if result not found in that search method
  COALESCE(1.0 / (rrf_k + full_text.rank_ix), 0.0) * full_text_weight +
  COALESCE(1.0 / (rrf_k + semantic.rank_ix), 0.0) * semantic_weight
  DESC,
  post.id ASC
LIMIT match_count
OFFSET offset_count
$$;

-- ============================================================================
-- Hybrid search count function for pagination metadata
-- ============================================================================

-- Count total results matching hybrid search criteria
-- Used to calculate total pages for pagination UI
--
-- Parameters:
--   query_text: Search query string
--   query_embedding: 768-dimensional vector from embedding service
--   full_text_weight: Weight for full-text search (not used in count, for consistency)
--   semantic_weight: Weight for semantic search (not used in count, for consistency)
--   rrf_k: RRF constant (not used in count, for consistency)
CREATE OR REPLACE FUNCTION hybrid_search_count(
  query_text TEXT,
  query_embedding vector(768),
  full_text_weight FLOAT = 1,
  semantic_weight FLOAT = 1,
  rrf_k INT = 50
)
RETURNS BIGINT
LANGUAGE SQL
AS $$
-- Full-text search matches
WITH full_text AS (
  SELECT id
  FROM post
  WHERE content_tsv @@ websearch_to_tsquery(query_text)
),
-- Vector similarity search matches
semantic AS (
  SELECT id
  FROM post
  WHERE embedding <=> query_embedding < 0.8
)
-- Count unique posts from either search method
SELECT COUNT(DISTINCT COALESCE(full_text.id, semantic.id)) AS total_count
FROM full_text
FULL OUTER JOIN semantic ON full_text.id = semantic.id;
$$;
