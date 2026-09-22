-- 19 桁以上の ID がある場合は、外部キーやカラムを変更する前に中止する。
CREATE TEMPORARY TABLE discord_id_rollback_guard (
    id_length INT NOT NULL CHECK (id_length <= 18)
);
INSERT INTO discord_id_rollback_guard (id_length)
    SELECT CHAR_LENGTH(discord_id) FROM discord_linked_users
    UNION ALL
    SELECT CHAR_LENGTH(discord_id) FROM discord_notification_settings;
DROP TEMPORARY TABLE discord_id_rollback_guard;

ALTER TABLE discord_notification_settings
    DROP FOREIGN KEY fk_discord_notification_settings_id;

ALTER TABLE discord_linked_users MODIFY discord_id VARCHAR(18) NOT NULL;
ALTER TABLE discord_notification_settings MODIFY discord_id VARCHAR(18) NOT NULL;

ALTER TABLE discord_notification_settings
    ADD CONSTRAINT fk_discord_notification_settings_id
    FOREIGN KEY (discord_id) REFERENCES discord_linked_users(discord_id) ON DELETE CASCADE;
