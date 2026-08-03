-- 旧スキーマで欠損していた回答を timestamp 付き回答より後ろへ置くため、最古時刻へ正規化する。
UPDATE answers SET timestamp = '1970-01-01 00:00:01' WHERE timestamp IS NULL;
UPDATE archived_answers SET timestamp = '1970-01-01 00:00:01' WHERE timestamp IS NULL;

ALTER TABLE answers MODIFY timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE archived_answers MODIFY timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP;

CREATE INDEX idx_answers_timestamp_id ON answers (timestamp DESC, id DESC);
CREATE INDEX idx_answers_form_id_timestamp_id ON answers (form_id, timestamp DESC, id DESC);
