-- Add down migration script here

-- 1. 인덱스 삭제 (선택적이지만 명시적으로 작성)
DROP INDEX IF EXISTS idx_post_tag_tag_id;
DROP INDEX IF EXISTS idx_post_tag_post_id;

-- 2. post_tag 테이블 삭제 (외래키 참조가 있는 테이블을 먼저 삭제)
DROP TABLE IF EXISTS post_tag;

-- 3. tag 테이블 삭제
DROP TABLE IF EXISTS tag;
