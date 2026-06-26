ALTER TABLE answer_submission_restrictions
    DROP FOREIGN KEY fk_answer_submission_restrictions_user_id,
    DROP FOREIGN KEY fk_answer_submission_restrictions_restricted_by,
    DROP FOREIGN KEY fk_answer_submission_restrictions_lifted_by,
    DROP INDEX idx_answer_submission_restrictions_user_id,
    DROP INDEX idx_answer_submission_restrictions_active,
    RENAME COLUMN user_id TO submitter_id,
    ADD INDEX idx_answer_submitter_restrictions_submitter_id(submitter_id),
    ADD INDEX idx_answer_submitter_restrictions_active(submitter_id, lifted_at, expires_at),
    ADD CONSTRAINT fk_answer_submitter_restrictions_submitter_id FOREIGN KEY (submitter_id) REFERENCES users(id),
    ADD CONSTRAINT fk_answer_submitter_restrictions_restricted_by FOREIGN KEY (restricted_by) REFERENCES users(id),
    ADD CONSTRAINT fk_answer_submitter_restrictions_lifted_by FOREIGN KEY (lifted_by) REFERENCES users(id),
    RENAME TO answer_submitter_restrictions;
