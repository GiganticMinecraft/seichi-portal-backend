ALTER TABLE form_meta_data
    ADD COLUMN answer_response_visibility ENUM('FULL', 'RESTRICTED') NOT NULL DEFAULT 'FULL' AFTER answer_visibility;

ALTER TABLE archived_form_meta_data
    ADD COLUMN answer_response_visibility ENUM('FULL', 'RESTRICTED') NOT NULL DEFAULT 'FULL' AFTER answer_visibility;
