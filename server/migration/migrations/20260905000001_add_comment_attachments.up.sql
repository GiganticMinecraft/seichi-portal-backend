CREATE TABLE IF NOT EXISTS form_answer_comment_attachments(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    comment_id CHAR(36) NOT NULL,
    file_name TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_form_answer_comment_attachments_comment_id(comment_id),
    INDEX idx_form_answer_comment_attachments_answer_id(answer_id),
    FOREIGN KEY fk_form_answer_comment_attachments_answer_id(answer_id)
        REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_form_answer_comment_attachments_comment_id(comment_id)
        REFERENCES form_answer_comments(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_answer_comment_attachments(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    comment_id CHAR(36) NOT NULL,
    file_name TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_archived_form_answer_comment_attachments_comment_id(comment_id),
    INDEX idx_archived_form_answer_comment_attachments_answer_id(answer_id),
    FOREIGN KEY fk_archived_form_answer_comment_attachments_answer_id(answer_id)
        REFERENCES archived_answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_form_answer_comment_attachments_comment_id(comment_id)
        REFERENCES archived_form_answer_comments(id) ON DELETE CASCADE
);
