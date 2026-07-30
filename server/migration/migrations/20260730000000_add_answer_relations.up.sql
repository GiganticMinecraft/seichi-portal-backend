CREATE TABLE answer_identities (
    answer_id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    INDEX idx_answer_identities_form_id_answer_id (form_id, answer_id)
);

INSERT INTO answer_identities (answer_id, form_id)
SELECT id, form_id FROM answers
ON DUPLICATE KEY UPDATE form_id = VALUES(form_id);

INSERT INTO answer_identities (answer_id, form_id)
SELECT id, form_id FROM archived_answers
ON DUPLICATE KEY UPDATE form_id = VALUES(form_id);

CREATE TABLE answer_relations (
    answer_id_first CHAR(36) NOT NULL,
    answer_id_second CHAR(36) NOT NULL,
    PRIMARY KEY (answer_id_first, answer_id_second),
    CONSTRAINT fk_answer_relations_first FOREIGN KEY (answer_id_first)
        REFERENCES answer_identities(answer_id),
    CONSTRAINT fk_answer_relations_second FOREIGN KEY (answer_id_second)
        REFERENCES answer_identities(answer_id),
    CONSTRAINT chk_answer_relations_distinct CHECK (answer_id_first <> answer_id_second),
    CONSTRAINT chk_answer_relations_normalized CHECK (answer_id_first < answer_id_second)
);
