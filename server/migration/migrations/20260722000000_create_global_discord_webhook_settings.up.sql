CREATE TABLE global_discord_webhook_settings(
    singleton_key TINYINT NOT NULL DEFAULT 1 PRIMARY KEY,
    url TEXT,
    CONSTRAINT chk_global_discord_webhook_singleton CHECK (singleton_key = 1)
);

INSERT INTO global_discord_webhook_settings (singleton_key, url) VALUES (1, NULL);
