CREATE TEMPORARY TABLE form_revision_rollback_guard(
    safe BOOL NOT NULL CHECK (safe)
);
INSERT INTO form_revision_rollback_guard (safe)
SELECT NOT EXISTS(
    SELECT 1
    FROM real_answers r
    LEFT JOIN form_questions q ON q.question_id = r.question_id
    WHERE q.question_id IS NULL
    UNION ALL
    SELECT 1
    FROM archived_real_answers r
    LEFT JOIN archived_form_questions q ON q.question_id = r.question_id
    WHERE q.question_id IS NULL
);
DROP TEMPORARY TABLE form_revision_rollback_guard;

ALTER TABLE archived_real_answers
    DROP FOREIGN KEY fk_archived_real_answers_revision_question,
    DROP FOREIGN KEY fk_archived_real_answers_answer_revision;
ALTER TABLE archived_real_answers
    DROP COLUMN form_revision_id,
    ADD CONSTRAINT fk_archived_real_answers_answer_id FOREIGN KEY(answer_id)
        REFERENCES archived_answers(id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_archived_real_answers_question_id FOREIGN KEY(question_id)
        REFERENCES archived_form_questions(question_id) ON DELETE CASCADE;

ALTER TABLE real_answers
    DROP FOREIGN KEY fk_real_answers_revision_question,
    DROP FOREIGN KEY fk_real_answers_answer_revision;
ALTER TABLE real_answers
    DROP COLUMN form_revision_id,
    ADD CONSTRAINT fk_real_answers_answer_id FOREIGN KEY(answer_id)
        REFERENCES answers(id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_real_answers_question_id FOREIGN KEY(question_id)
        REFERENCES form_questions(question_id) ON DELETE CASCADE;

ALTER TABLE archived_answers
    DROP FOREIGN KEY fk_archived_answers_form_revision,
    DROP INDEX uk_archived_answers_id_revision,
    DROP COLUMN form_revision_id;
ALTER TABLE answers
    DROP FOREIGN KEY fk_answers_form_revision,
    DROP INDEX uk_answers_id_revision,
    DROP COLUMN form_revision_id;

DROP TABLE archived_form_revision_choices;
DROP TABLE archived_form_revision_questions;
DROP TABLE archived_form_revisions;
DROP TABLE form_revision_choices;
DROP TABLE form_revision_questions;
DROP TABLE form_revisions;
