ALTER TABLE archived_form_meta_data
    DROP COLUMN answer_entry_set_id;

DROP TABLE IF EXISTS archived_answer_entry_sets;

ALTER TABLE form_meta_data
    DROP FOREIGN KEY fk_form_meta_data_answer_entry_set_id,
    DROP COLUMN answer_entry_set_id;

DROP TABLE IF EXISTS answer_entry_sets;
