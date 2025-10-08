-- Add down migration script here
-- hybrid_search 함수 삭제
DROP FUNCTION IF EXISTS hybrid_search(
  query_text TEXT,
  query_embedding vector(768),
  match_count INT,
  full_text_weight FLOAT,
  semantic_weight FLOAT,
  rrf_k INT
);

-- 인덱스 삭제
DROP INDEX IF EXISTS post_tsv_gin_idx;
DROP INDEX IF EXISTS post_embedding_hnsw_idx;

