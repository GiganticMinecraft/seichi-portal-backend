-- 別の Discord アカウントへ連携し直すと discord_linked_users.discord_id が変わる。
-- 通知設定の外部キーが更新に追従しないと、設定がある利用者は連携し直せない。
ALTER TABLE discord_notification_settings
    DROP FOREIGN KEY fk_discord_notification_settings_id;

ALTER TABLE discord_notification_settings
    ADD CONSTRAINT fk_discord_notification_settings_id
    FOREIGN KEY (discord_id) REFERENCES discord_linked_users(discord_id)
    ON DELETE CASCADE ON UPDATE CASCADE;
