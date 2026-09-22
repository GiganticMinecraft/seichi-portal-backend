ALTER TABLE discord_notification_settings
    DROP FOREIGN KEY fk_discord_notification_settings_id;

ALTER TABLE discord_linked_users MODIFY discord_id VARCHAR(20) NOT NULL;
ALTER TABLE discord_notification_settings MODIFY discord_id VARCHAR(20) NOT NULL;

ALTER TABLE discord_notification_settings
    ADD CONSTRAINT fk_discord_notification_settings_id
    FOREIGN KEY (discord_id) REFERENCES discord_linked_users(discord_id) ON DELETE CASCADE;
