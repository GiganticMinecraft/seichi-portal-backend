CREATE TABLE IF NOT EXISTS message_threads (
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL UNIQUE,
    answer_author_id CHAR(36) NOT NULL,
    FOREIGN KEY (answer_id) REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY (answer_author_id) REFERENCES users(id)
);
