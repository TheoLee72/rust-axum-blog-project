-- Add up migration script here

CREATE INDEX post_tsv_gin_idx ON post USING gin (content_tsv);

CREATE INDEX post_embedding_hnsw_idx ON post USING hnsw (embedding vector_cosine_ops);

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
WITH full_text AS (
  SELECT
    id,
    ROW_NUMBER() OVER (ORDER BY ts_rank_cd(content_tsv, websearch_to_tsquery(query_text)) DESC, id ASC) AS rank_ix
  FROM
    post
  WHERE
    content_tsv @@ websearch_to_tsquery(query_text)
  LIMIT (match_count + offset_count) * 2
),
semantic AS (
  SELECT
    id,
    ROW_NUMBER() OVER (ORDER BY embedding <=> query_embedding, id ASC) AS rank_ix
  FROM
    post
  WHERE
    embedding <=> query_embedding < 0.8
  LIMIT (match_count + offset_count) * 2
)
SELECT
  post.*
FROM
  full_text
  FULL OUTER JOIN semantic ON full_text.id = semantic.id
  JOIN post ON COALESCE(full_text.id, semantic.id) = post.id
ORDER BY
  COALESCE(1.0 / (rrf_k + full_text.rank_ix), 0.0) * full_text_weight +
  COALESCE(1.0 / (rrf_k + semantic.rank_ix), 0.0) * semantic_weight
  DESC,
  post.id ASC
LIMIT match_count
OFFSET offset_count
$$;


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
WITH full_text AS (
  SELECT id
  FROM post
  WHERE content_tsv @@ websearch_to_tsquery(query_text)
),
semantic AS (
  SELECT id
  FROM post
  WHERE embedding <=> query_embedding < 0.8
)
SELECT COUNT(DISTINCT COALESCE(full_text.id, semantic.id)) AS total_count
FROM full_text
FULL OUTER JOIN semantic ON full_text.id = semantic.id;
$$;




