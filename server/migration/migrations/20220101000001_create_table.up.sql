CREATE TABLE IF NOT EXISTS users(
    id CHAR(36) NOT NULL PRIMARY KEY,
    name VARCHAR(16) NOT NULL,
    role ENUM('ADMINISTRATOR', 'STANDARD_USER') NOT NULL
);

CREATE TABLE IF NOT EXISTS answer_submitter_restrictions(
    id CHAR(36) NOT NULL PRIMARY KEY,
    submitter_id CHAR(36) NOT NULL,
    reason TEXT NOT NULL,
    restricted_by CHAR(36) NOT NULL,
    restricted_at DATETIME(6) NOT NULL,
    expires_at DATETIME(6),
    lifted_at DATETIME(6),
    lifted_by CHAR(36),
    INDEX idx_answer_submitter_restrictions_submitter_id(submitter_id),
    INDEX idx_answer_submitter_restrictions_active(submitter_id, lifted_at, expires_at),
    FOREIGN KEY fk_answer_submitter_restrictions_submitter_id(submitter_id) REFERENCES users(id),
    FOREIGN KEY fk_answer_submitter_restrictions_restricted_by(restricted_by) REFERENCES users(id),
    FOREIGN KEY fk_answer_submitter_restrictions_lifted_by(lifted_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS temporary_users(
    id CHAR(36) NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    contact_text TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS discord_linked_users(
    user_id CHAR(36) NOT NULL PRIMARY KEY,
    discord_id VARCHAR(18) NOT NULL UNIQUE,
    discord_username VARCHAR(32) NOT NULL,
    FOREIGN KEY fk_discord_linked_users_id(user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS discord_notification_settings(
    discord_id VARCHAR(18) NOT NULL PRIMARY KEY,
    is_send_message_notification BOOL NOT NULL,
    FOREIGN KEY fk_discord_notification_settings_id(discord_id) REFERENCES discord_linked_users(discord_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_meta_data(
    id CHAR(36) NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
    allow_temporary_answers BOOL NOT NULL DEFAULT FALSE,
    answer_visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
    acceptance_period_start_at DATETIME,
    acceptance_period_end_at DATETIME,
    default_answer_title TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by CHAR(36) NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    updated_by CHAR(36) NOT NULL,
    FOREIGN KEY fk_form_meta_data_created_by(created_by) REFERENCES users(id),
    FOREIGN KEY fk_form_meta_data_updated_by(updated_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS form_questions(
    question_id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    template_key VARCHAR(255) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    question_type VARCHAR(32) NOT NULL,
    is_required BOOL DEFAULT FALSE,
    UNIQUE KEY uk_form_questions_form_id_template_key(form_id, template_key),
    UNIQUE KEY uk_form_questions_form_id_position(form_id, position),
    FOREIGN KEY fk_form_questions_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_choices(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    question_id CHAR(36) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    label TEXT NOT NULL,
    UNIQUE KEY uk_form_choices_question_id_position(question_id, position),
    FOREIGN KEY fk_form_choices_question_id(question_id) REFERENCES form_questions(question_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_discord_webhooks(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    url TEXT,
    UNIQUE KEY uk_form_discord_webhooks_form_id(form_id),
    FOREIGN KEY fk_form_discord_webhooks_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS answers(
    id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    author_type ENUM('AUTHENTICATED_USER', 'TEMPORARY_USER') NOT NULL,
    user CHAR(36),
    temporary_user_id CHAR(36),
    title TEXT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY fk_answers_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_answers_user(user) REFERENCES users(id),
    FOREIGN KEY fk_answers_temporary_user_id(temporary_user_id) REFERENCES temporary_users(id),
    CHECK (
        (author_type = 'AUTHENTICATED_USER' AND user IS NOT NULL AND temporary_user_id IS NULL)
        OR (author_type = 'TEMPORARY_USER' AND user IS NULL AND temporary_user_id IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS real_answers(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    question_id CHAR(36) NOT NULL,
    answer TEXT NOT NULL,
    FOREIGN KEY fk_real_answers_answer_id(answer_id) REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_real_answers_question_id(question_id) REFERENCES form_questions(question_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_answer_comments(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    commented_by CHAR(36) NOT NULL,
    content TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY fk_form_answer_comments_answer_id(answer_id) REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_form_answer_comments_commented_by(commented_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS label_for_form_answers(
    id CHAR(36) NOT NULL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS label_settings_for_form_answers(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    label_id CHAR(36) NOT NULL,
    FOREIGN KEY fk_label_settings_for_form_answers_answer_id(answer_id) REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_label_settings_for_form_answers_label_id(label_id) REFERENCES label_for_form_answers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_for_forms(
    id CHAR(36) NOT NULL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS label_settings_for_forms(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    label_id CHAR(36) NOT NULL,
    FOREIGN KEY fk_label_settings_for_forms_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_label_settings_for_forms_label_id(label_id) REFERENCES label_for_forms(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages(
    id CHAR(36) NOT NULL PRIMARY KEY,
    related_answer_id CHAR(36) NOT NULL,
    sender CHAR(36) NOT NULL,
    body TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY fk_message_related_answer_id(related_answer_id) REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_message_sender(sender) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS archived_form_meta_data(
    id CHAR(36) NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
    allow_temporary_answers BOOL NOT NULL DEFAULT FALSE,
    answer_visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
    acceptance_period_start_at DATETIME,
    acceptance_period_end_at DATETIME,
    default_answer_title TEXT,
    created_at DATETIME NOT NULL,
    created_by CHAR(36) NOT NULL,
    updated_at DATETIME NOT NULL,
    updated_by CHAR(36) NOT NULL,
    archived_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    archived_by CHAR(36) NOT NULL,
    FOREIGN KEY fk_archived_form_meta_data_created_by(created_by) REFERENCES users(id),
    FOREIGN KEY fk_archived_form_meta_data_updated_by(updated_by) REFERENCES users(id),
    FOREIGN KEY fk_archived_form_meta_data_archived_by(archived_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS archived_form_questions(
    question_id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    template_key VARCHAR(255) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    question_type VARCHAR(32) NOT NULL,
    is_required BOOL DEFAULT FALSE,
    UNIQUE KEY uk_archived_form_questions_form_id_template_key(form_id, template_key),
    UNIQUE KEY uk_archived_form_questions_form_id_position(form_id, position),
    FOREIGN KEY fk_archived_form_questions_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_choices(
    id INT NOT NULL PRIMARY KEY,
    question_id CHAR(36) NOT NULL,
    position SMALLINT UNSIGNED NOT NULL,
    label TEXT NOT NULL,
    UNIQUE KEY uk_archived_form_choices_question_id_position(question_id, position),
    FOREIGN KEY fk_archived_form_choices_question_id(question_id) REFERENCES archived_form_questions(question_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_discord_webhooks(
    id INT NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    url TEXT,
    UNIQUE KEY uk_archived_form_discord_webhooks_form_id(form_id),
    FOREIGN KEY fk_archived_form_discord_webhooks_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_answers(
    id CHAR(36) NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    author_type ENUM('AUTHENTICATED_USER', 'TEMPORARY_USER') NOT NULL,
    user CHAR(36),
    temporary_user_id CHAR(36),
    title TEXT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY fk_archived_answers_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_answers_user(user) REFERENCES users(id),
    FOREIGN KEY fk_archived_answers_temporary_user_id(temporary_user_id) REFERENCES temporary_users(id),
    CHECK (
        (author_type = 'AUTHENTICATED_USER' AND user IS NOT NULL AND temporary_user_id IS NULL)
        OR (author_type = 'TEMPORARY_USER' AND user IS NULL AND temporary_user_id IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS archived_real_answers(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    question_id CHAR(36) NOT NULL,
    answer TEXT NOT NULL,
    FOREIGN KEY fk_archived_real_answers_answer_id(answer_id) REFERENCES archived_answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_real_answers_question_id(question_id) REFERENCES archived_form_questions(question_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_answer_comments(
    id CHAR(36) NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    commented_by CHAR(36) NOT NULL,
    content TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY fk_archived_form_answer_comments_answer_id(answer_id) REFERENCES archived_answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_form_answer_comments_commented_by(commented_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS archived_messages(
    id CHAR(36) NOT NULL PRIMARY KEY,
    related_answer_id CHAR(36) NOT NULL,
    sender CHAR(36) NOT NULL,
    body TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY fk_archived_messages_related_answer_id(related_answer_id) REFERENCES archived_answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_messages_sender(sender) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS archived_label_settings_for_forms(
    id INT NOT NULL PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    label_id CHAR(36) NOT NULL,
    FOREIGN KEY fk_archived_label_settings_for_forms_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_label_settings_for_forms_label_id(label_id) REFERENCES label_for_forms(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_label_settings_for_form_answers(
    id INT NOT NULL PRIMARY KEY,
    answer_id CHAR(36) NOT NULL,
    label_id CHAR(36) NOT NULL,
    FOREIGN KEY fk_archived_label_settings_for_form_answers_answer_id(answer_id) REFERENCES archived_answers(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_label_settings_for_form_answers_label_id(label_id) REFERENCES label_for_form_answers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS message_threads (
    answer_id CHAR(36) NOT NULL PRIMARY KEY,
    answer_author_id CHAR(36) NOT NULL,
    FOREIGN KEY (answer_id) REFERENCES answers(id) ON DELETE CASCADE,
    FOREIGN KEY (answer_author_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS user_groups(
    id CHAR(36) NOT NULL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS user_group_memberships(
    group_id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    PRIMARY KEY (group_id, user_id),
    INDEX idx_user_group_memberships_user_id(user_id),
    FOREIGN KEY fk_user_group_memberships_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE,
    FOREIGN KEY fk_user_group_memberships_user_id(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_allowed_user_groups(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    group_id CHAR(36) NOT NULL,
    UNIQUE KEY uk_form_allowed_user_groups(form_id, group_id),
    FOREIGN KEY fk_form_allowed_user_groups_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_form_allowed_user_groups_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_answer_submitter_groups(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    group_id CHAR(36) NOT NULL,
    UNIQUE KEY uk_form_answer_submitter_groups(form_id, group_id),
    FOREIGN KEY fk_form_answer_submitter_groups_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_form_answer_submitter_groups_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS form_answer_reader_groups(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    group_id CHAR(36) NOT NULL,
    UNIQUE KEY uk_form_answer_reader_groups(form_id, group_id),
    FOREIGN KEY fk_form_answer_reader_groups_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_form_answer_reader_groups_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_allowed_user_groups(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    group_id CHAR(36) NOT NULL,
    UNIQUE KEY uk_archived_form_allowed_user_groups(form_id, group_id),
    FOREIGN KEY fk_archived_form_allowed_user_groups_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_form_allowed_user_groups_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_answer_submitter_groups(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    group_id CHAR(36) NOT NULL,
    UNIQUE KEY uk_archived_form_answer_submitter_groups(form_id, group_id),
    FOREIGN KEY fk_archived_form_answer_submitter_groups_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_form_answer_submitter_groups_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS archived_form_answer_reader_groups(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    group_id CHAR(36) NOT NULL,
    UNIQUE KEY uk_archived_form_answer_reader_groups(form_id, group_id),
    FOREIGN KEY fk_archived_form_answer_reader_groups_form_id(form_id) REFERENCES archived_form_meta_data(id) ON DELETE CASCADE,
    FOREIGN KEY fk_archived_form_answer_reader_groups_group_id(group_id) REFERENCES user_groups(id) ON DELETE CASCADE
);
