DROP INDEX idx_answers_timestamp_id ON answers;
DROP INDEX idx_answers_form_id_timestamp_id ON answers;

ALTER TABLE answers MODIFY timestamp TIMESTAMP NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE archived_answers MODIFY timestamp TIMESTAMP NULL DEFAULT CURRENT_TIMESTAMP;

-- upで正規化したNULL値は復元できない。
