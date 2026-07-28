INSERT INTO litebans_bans (
    id, uuid, ip, reason, banned_by_uuid, banned_by_name,
    removed_by_uuid, removed_by_name, `time`, `until`, server_scope, server_origin,
    silent, ipban, ipban_wildcard, active, removed_by_reason, template
)
VALUES
    (
        900001, '5cb955fb-5a05-4729-93ea-edcec7001001', NULL, 'debug temporary ban',
        NULL, 'DebugModerator', NULL, NULL, 1700000000123, 1800000000456, NULL, 'local',
        b'0', b'0', b'0', b'1', NULL, 255
    ),
    (
        900002, '5cb955fb-5a05-4729-93ea-edcec7001001', NULL, 'debug permanent IP ban',
        NULL, 'DebugModerator', NULL, NULL, 1690000000123, 0, NULL, 'local',
        b'0', b'1', b'0', b'1', NULL, 255
    )
ON DUPLICATE KEY UPDATE
    uuid = VALUES(uuid),
    ip = VALUES(ip),
    reason = VALUES(reason),
    banned_by_uuid = VALUES(banned_by_uuid),
    banned_by_name = VALUES(banned_by_name),
    removed_by_uuid = VALUES(removed_by_uuid),
    removed_by_name = VALUES(removed_by_name),
    `time` = VALUES(`time`),
    `until` = VALUES(`until`),
    server_scope = VALUES(server_scope),
    server_origin = VALUES(server_origin),
    silent = VALUES(silent),
    ipban = VALUES(ipban),
    ipban_wildcard = VALUES(ipban_wildcard),
    active = VALUES(active),
    removed_by_reason = VALUES(removed_by_reason),
    template = VALUES(template);
