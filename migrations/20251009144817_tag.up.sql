-- Add up migration script here

CREATE TABLE tag (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) UNIQUE
);

CREATE TABLE post_tag (
    post_id INTEGER REFERENCES post(id) ON DELETE CASCADE,
    tag_id INTEGER REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);

CREATE INDEX idx_post_tag_post_id ON post_tag(post_id);
CREATE INDEX idx_post_tag_tag_id ON post_tag(tag_id);