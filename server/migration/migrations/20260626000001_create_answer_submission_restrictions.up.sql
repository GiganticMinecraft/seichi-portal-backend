CREATE TABLE IF NOT EXISTS answer_submission_restrictions(
    id CHAR(36) NOT NULL PRIMARY KEY,
    user_id CHAR(36) NOT NULL,
    reason TEXT NOT NULL,
    restricted_by CHAR(36) NOT NULL,
    restricted_at DATETIME(6) NOT NULL,
    expires_at DATETIME(6),
    lifted_at DATETIME(6),
    lifted_by CHAR(36),
    INDEX idx_answer_submission_restrictions_user_id(user_id),
    INDEX idx_answer_submission_restrictions_active(user_id, lifted_at, expires_at),
    FOREIGN KEY fk_answer_submission_restrictions_user_id(user_id) REFERENCES users(id),
    FOREIGN KEY fk_answer_submission_restrictions_restricted_by(restricted_by) REFERENCES users(id),
    FOREIGN KEY fk_answer_submission_restrictions_lifted_by(lifted_by) REFERENCES users(id)
);
