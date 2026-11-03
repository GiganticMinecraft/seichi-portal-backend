-- 回答はフォームのアーカイブ時に answers と archived_answers の間を移動する。
-- answers への外部キーを ON DELETE CASCADE で持っていると、アーカイブのたびに
-- 通知が消え、復元しても戻らない。answer_relations と同様に外部キーを持たず、
-- 安定した UUID だけを保存する。
ALTER TABLE notifications
    DROP FOREIGN KEY fk_notifications_related_answer_id;
