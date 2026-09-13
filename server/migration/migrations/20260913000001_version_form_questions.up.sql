CREATE TABLE form_revisions(
    id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    revision_number INT UNSIGNED NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_form_revisions_form_id_revision_number(form_id, revision_number),
    UNIQUE KEY uk_form_revisions_form_id_id(form_id, id),
    FOREIGN KEY fk_form_revisions_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
);

CREATE TABLE form_revision_questions(
    form_revision_id CHAR(36) NOT NULL,
    question_id CHAR(36) NOT NULL,
    template_key VARCHAR(255) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    question_type VARCHAR(32) NOT NULL,
    is_required BOOL DEFAULT FALSE,
    PRIMARY KEY(form_revision_id, question_id),
    UNIQUE KEY uk_form_revision_questions_template_key(form_revision_id, template_key),
    UNIQUE KEY uk_form_revision_questions_position(form_revision_id, position),
    FOREIGN KEY fk_form_revision_questions_revision(form_revision_id)
        REFERENCES form_revisions(id) ON DELETE CASCADE
);

CREATE TABLE form_revision_choices(
    form_revision_id CHAR(36) NOT NULL,
    id INT NOT NULL,
    question_id CHAR(36) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY(form_revision_id, id),
    UNIQUE KEY uk_form_revision_choices_position(form_revision_id, question_id, position),
    FOREIGN KEY fk_form_revision_choices_question(form_revision_id, question_id)
        REFERENCES form_revision_questions(form_revision_id, question_id) ON DELETE CASCADE
);

CREATE TABLE archived_form_revisions(
    id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    revision_number INT UNSIGNED NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_archived_form_revisions_form_id_revision_number(form_id, revision_number),
    UNIQUE KEY uk_archived_form_revisions_form_id_id(form_id, id),
    FOREIGN KEY fk_archived_form_revisions_form_id(form_id)
        REFERENCES archived_form_meta_data(id) ON DELETE CASCADE
);

CREATE TABLE archived_form_revision_questions(
    form_revision_id CHAR(36) NOT NULL,
    question_id CHAR(36) NOT NULL,
    template_key VARCHAR(255) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    question_type VARCHAR(32) NOT NULL,
    is_required BOOL DEFAULT FALSE,
    PRIMARY KEY(form_revision_id, question_id),
    UNIQUE KEY uk_archived_form_revision_questions_template_key(form_revision_id, template_key),
    UNIQUE KEY uk_archived_form_revision_questions_position(form_revision_id, position),
    FOREIGN KEY fk_archived_form_revision_questions_revision(form_revision_id)
        REFERENCES archived_form_revisions(id) ON DELETE CASCADE
);

CREATE TABLE archived_form_revision_choices(
    form_revision_id CHAR(36) NOT NULL,
    id INT NOT NULL,
    question_id CHAR(36) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY(form_revision_id, id),
    UNIQUE KEY uk_archived_form_revision_choices_position(form_revision_id, question_id, position),
    FOREIGN KEY fk_archived_form_revision_choices_question(form_revision_id, question_id)
        REFERENCES archived_form_revision_questions(form_revision_id, question_id) ON DELETE CASCADE
);

INSERT INTO form_revisions (id, form_id, revision_number, created_at)
SELECT UUID(), id, 1, created_at FROM form_meta_data;

INSERT INTO form_revision_questions
    (form_revision_id, question_id, template_key, position, title, description, question_type, is_required)
SELECT r.id, q.question_id, q.template_key, q.position, q.title, q.description,
    q.question_type, q.is_required
FROM form_questions q
INNER JOIN form_revisions r ON r.form_id = q.form_id;

INSERT INTO form_revision_choices (form_revision_id, id, question_id, position, label)
SELECT r.id, c.id, c.question_id, c.position, c.label
FROM form_choices c
INNER JOIN form_questions q ON q.question_id = c.question_id
INNER JOIN form_revisions r ON r.form_id = q.form_id;

INSERT INTO archived_form_revisions (id, form_id, revision_number, created_at)
SELECT UUID(), id, 1, created_at FROM archived_form_meta_data;

INSERT INTO archived_form_revision_questions
    (form_revision_id, question_id, template_key, position, title, description, question_type, is_required)
SELECT r.id, q.question_id, q.template_key, q.position, q.title, q.description,
    q.question_type, q.is_required
FROM archived_form_questions q
INNER JOIN archived_form_revisions r ON r.form_id = q.form_id;

INSERT INTO archived_form_revision_choices (form_revision_id, id, question_id, position, label)
SELECT r.id, c.id, c.question_id, c.position, c.label
FROM archived_form_choices c
INNER JOIN archived_form_questions q ON q.question_id = c.question_id
INNER JOIN archived_form_revisions r ON r.form_id = q.form_id;

ALTER TABLE answers ADD COLUMN form_revision_id CHAR(36) NULL AFTER form_id;
UPDATE answers a
INNER JOIN form_revisions r ON r.form_id = a.form_id
SET a.form_revision_id = r.id;
ALTER TABLE answers
    MODIFY COLUMN form_revision_id CHAR(36) NOT NULL,
    ADD UNIQUE KEY uk_answers_id_revision(id, form_revision_id),
    ADD CONSTRAINT fk_answers_form_revision FOREIGN KEY(form_id, form_revision_id)
        REFERENCES form_revisions(form_id, id) ON DELETE RESTRICT;

ALTER TABLE archived_answers ADD COLUMN form_revision_id CHAR(36) NULL AFTER form_id;
UPDATE archived_answers a
INNER JOIN archived_form_revisions r ON r.form_id = a.form_id
SET a.form_revision_id = r.id;
ALTER TABLE archived_answers
    MODIFY COLUMN form_revision_id CHAR(36) NOT NULL,
    ADD UNIQUE KEY uk_archived_answers_id_revision(id, form_revision_id),
    ADD CONSTRAINT fk_archived_answers_form_revision FOREIGN KEY(form_id, form_revision_id)
        REFERENCES archived_form_revisions(form_id, id) ON DELETE RESTRICT;

ALTER TABLE real_answers ADD COLUMN form_revision_id CHAR(36) NULL AFTER answer_id;
UPDATE real_answers r
INNER JOIN answers a ON a.id = r.answer_id
SET r.form_revision_id = a.form_revision_id;
ALTER TABLE real_answers
    MODIFY COLUMN form_revision_id CHAR(36) NOT NULL,
    DROP FOREIGN KEY fk_real_answers_answer_id,
    DROP FOREIGN KEY fk_real_answers_question_id,
    ADD CONSTRAINT fk_real_answers_answer_revision FOREIGN KEY(answer_id, form_revision_id)
        REFERENCES answers(id, form_revision_id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_real_answers_revision_question FOREIGN KEY(form_revision_id, question_id)
        REFERENCES form_revision_questions(form_revision_id, question_id) ON DELETE RESTRICT;

ALTER TABLE archived_real_answers ADD COLUMN form_revision_id CHAR(36) NULL AFTER answer_id;
UPDATE archived_real_answers r
INNER JOIN archived_answers a ON a.id = r.answer_id
SET r.form_revision_id = a.form_revision_id;
ALTER TABLE archived_real_answers
    MODIFY COLUMN form_revision_id CHAR(36) NOT NULL,
    DROP FOREIGN KEY fk_archived_real_answers_answer_id,
    DROP FOREIGN KEY fk_archived_real_answers_question_id,
    ADD CONSTRAINT fk_archived_real_answers_answer_revision FOREIGN KEY(answer_id, form_revision_id)
        REFERENCES archived_answers(id, form_revision_id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_archived_real_answers_revision_question FOREIGN KEY(form_revision_id, question_id)
        REFERENCES archived_form_revision_questions(form_revision_id, question_id) ON DELETE RESTRICT;
