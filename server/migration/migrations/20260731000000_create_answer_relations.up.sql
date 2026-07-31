-- 回答は archive 時に answers と archived_answers の間を移動するため、
-- 関連は answers への外部キーを持たず、安定した UUID の組だけを保存する。
CREATE TABLE answer_relations (
    first_form_id CHAR(36) NOT NULL,
    first_answer_id CHAR(36) NOT NULL,
    second_form_id CHAR(36) NOT NULL,
    second_answer_id CHAR(36) NOT NULL,
    PRIMARY KEY (first_form_id, first_answer_id, second_form_id, second_answer_id),
    INDEX idx_answer_relations_first (first_form_id, first_answer_id),
    INDEX idx_answer_relations_second (second_form_id, second_answer_id),
    CONSTRAINT chk_answer_relations_distinct
        CHECK (first_form_id <> second_form_id OR first_answer_id <> second_answer_id),
    CONSTRAINT chk_answer_relations_normalized
        CHECK (
            first_form_id < second_form_id
            OR (first_form_id = second_form_id AND first_answer_id < second_answer_id)
        )
);
