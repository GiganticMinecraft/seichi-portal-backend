-- 対応する回答が answers に存在しない通知は外部キーを張れないため先に消す。
DELETE FROM notifications
    WHERE related_answer_id NOT IN (SELECT id FROM answers);

ALTER TABLE notifications
    ADD CONSTRAINT fk_notifications_related_answer_id
    FOREIGN KEY (related_answer_id) REFERENCES answers(id) ON DELETE CASCADE;
