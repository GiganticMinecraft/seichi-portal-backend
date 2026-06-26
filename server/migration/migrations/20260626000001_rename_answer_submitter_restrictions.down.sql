ALTER TABLE answer_submitter_restrictions
    DROP FOREIGN KEY fk_answer_submitter_restrictions_submitter_id,
    DROP FOREIGN KEY fk_answer_submitter_restrictions_restricted_by,
    DROP FOREIGN KEY fk_answer_submitter_restrictions_lifted_by,
    DROP INDEX idx_answer_submitter_restrictions_submitter_id,
    DROP INDEX idx_answer_submitter_restrictions_active,
    RENAME COLUMN submitter_id TO user_id,
    ADD INDEX idx_answer_submission_restrictions_user_id(user_id),
    ADD INDEX idx_answer_submission_restrictions_active(user_id, lifted_at, expires_at),
    ADD CONSTRAINT fk_answer_submission_restrictions_user_id FOREIGN KEY (user_id) REFERENCES users(id),
    ADD CONSTRAINT fk_answer_submission_restrictions_restricted_by FOREIGN KEY (restricted_by) REFERENCES users(id),
    ADD CONSTRAINT fk_answer_submission_restrictions_lifted_by FOREIGN KEY (lifted_by) REFERENCES users(id),
    RENAME TO answer_submission_restrictions;
