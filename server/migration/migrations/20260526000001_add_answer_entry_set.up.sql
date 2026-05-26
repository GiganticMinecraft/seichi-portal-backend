CREATE TABLE IF NOT EXISTS answer_entry_sets(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
    response_period_start_at DATETIME,
    response_period_end_at DATETIME,
    allow_temporary_answers BOOL NOT NULL DEFAULT FALSE,
    default_answer_title TEXT
);

INSERT INTO answer_entry_sets (id, answer_visibility, allow_temporary_answers)
SELECT UUID(), f.answer_visibility, f.allow_temporary_answers
FROM form_meta_data f;

ALTER TABLE form_meta_data
    ADD COLUMN answer_entry_set_id CHAR(36) AFTER answer_visibility;

UPDATE form_meta_data f
INNER JOIN answer_entry_sets a
    ON f.allow_temporary_answers = a.allow_temporary_answers
    AND f.answer_visibility = a.answer_visibility
SET f.answer_entry_set_id = a.id;

ALTER TABLE form_meta_data
    MODIFY COLUMN answer_entry_set_id CHAR(36) NOT NULL,
    ADD CONSTRAINT fk_form_meta_data_answer_entry_set_id
        FOREIGN KEY (answer_entry_set_id) REFERENCES answer_entry_sets(id);

UPDATE answer_entry_sets a
INNER JOIN form_meta_data f ON f.answer_entry_set_id = a.id
LEFT JOIN response_period rp ON rp.form_id = f.id
LEFT JOIN default_answer_titles dat ON dat.form_id = f.id
SET a.response_period_start_at = rp.start_at,
    a.response_period_end_at = rp.end_at,
    a.default_answer_title = dat.title;

ALTER TABLE archived_form_meta_data
    ADD COLUMN answer_entry_set_id CHAR(36) NOT NULL DEFAULT '' AFTER answer_visibility;

UPDATE archived_form_meta_data af
SET af.answer_entry_set_id = UUID();

CREATE TABLE IF NOT EXISTS archived_answer_entry_sets(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
    response_period_start_at DATETIME,
    response_period_end_at DATETIME,
    allow_temporary_answers BOOL NOT NULL DEFAULT FALSE,
    default_answer_title TEXT
);

INSERT INTO archived_answer_entry_sets (id, answer_visibility, allow_temporary_answers, response_period_start_at, response_period_end_at, default_answer_title)
SELECT af.answer_entry_set_id, af.answer_visibility, af.allow_temporary_answers, arp.start_at, arp.end_at, adat.title
FROM archived_form_meta_data af
LEFT JOIN archived_response_period arp ON arp.form_id = af.id
LEFT JOIN archived_default_answer_titles adat ON adat.form_id = af.id;
