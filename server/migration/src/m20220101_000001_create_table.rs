use sea_orm_migration::{prelude::*, sea_orm::Statement};

use crate::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS users(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    name VARCHAR(16) NOT NULL,
                    role ENUM('ADMINISTRATOR', 'STANDARD_USER') NOT NULL
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS discord_linked_users(
                    user_id CHAR(36) NOT NULL PRIMARY KEY,
                    discord_id VARCHAR(18) NOT NULL UNIQUE,
                    discord_username VARCHAR(32) NOT NULL,
                    FOREIGN KEY fk_discord_linked_users_id(user_id) REFERENCES users(id)
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS discord_notification_settings(
                    discord_id VARCHAR(18) NOT NULL PRIMARY KEY,
                    is_send_message_notification BOOL NOT NULL,
                    FOREIGN KEY fk_discord_notification_settings_id(discord_id) REFERENCES discord_linked_users(discord_id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_meta_data(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
                    answer_visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
                    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    created_by CHAR(36) NOT NULL,
                    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                    updated_by CHAR(36) NOT NULL,
                    FOREIGN KEY fk_form_meta_data_created_by(created_by) REFERENCES users(id),
                    FOREIGN KEY fk_form_meta_data_updated_by(updated_by) REFERENCES users(id)
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_questions(
                    question_id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    form_id CHAR(36) NOT NULL,
                    title TEXT NOT NULL,
                    description TEXT,
                    question_type ENUM('TEXT', 'SINGLE', 'MULTIPLE'),
                    is_required BOOL DEFAULT FALSE,
                    FOREIGN KEY fk_form_questions_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_choices(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    question_id INT NOT NULL,
                    choice TEXT NOT NULL,
                    FOREIGN KEY fk_form_choices_question_id(question_id) REFERENCES form_questions(question_id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS response_period(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    form_id CHAR(36) NOT NULL,
                    start_at DATETIME,
                    end_at DATETIME,
                    FOREIGN KEY fk_response_period_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_webhooks(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    form_id CHAR(36) NOT NULL,
                    url TEXT NOT NULL,
                    FOREIGN KEY fk_form_webhooks_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS answers(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    form_id CHAR(36) NOT NULL,
                    user CHAR(36) NOT NULL,
                    title TEXT,
                    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY fk_answers_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_answers_user(user) REFERENCES users(id)
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS real_answers(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    answer_id CHAR(36) NOT NULL,
                    question_id INT NOT NULL,
                    answer TEXT NOT NULL,
                    FOREIGN KEY fk_real_answers_answer_id(answer_id) REFERENCES answers(id),
                    FOREIGN KEY fk_real_answers_quesiton_id(question_id) REFERENCES form_questions(question_id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS default_answer_titles(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    form_id CHAR(36) NOT NULL,
                    title TEXT,
                    FOREIGN KEY fk_default_answer_titles_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_answer_comments(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    answer_id CHAR(36) NOT NULL,
                    commented_by CHAR(36) NOT NULL,
                    content TEXT NOT NULL,
                    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY fk_form_answer_comments_answer_id(answer_id) REFERENCES answers(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_form_answer_comments_commented_by(commented_by) REFERENCES users(id)
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS label_for_form_answers(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    name TEXT NOT NULL
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS label_settings_for_form_answers(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    answer_id CHAR(36) NOT NULL,
                    label_id CHAR(36) NOT NULL,
                    FOREIGN KEY fk_label_settings_for_form_answers_answer_id(answer_id) REFERENCES answers(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_label_settings_for_form_answers_label_id(label_id) REFERENCES label_for_form_answers(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS label_for_forms(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    name TEXT NOT NULL
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS label_settings_for_forms(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    form_id CHAR(36) NOT NULL,
                    label_id CHAR(36) NOT NULL,
                    FOREIGN KEY fk_label_settings_for_forms_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_label_settings_for_forms_label_id(label_id) REFERENCES label_for_forms(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS messages(
                    id CHAR(36) NOT NULL PRIMARY KEY,
                    related_answer_id CHAR(36) NOT NULL,
                    sender CHAR(36) NOT NULL,
                    body TEXT NOT NULL,
                    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY fk_message_related_answer_id(related_answer_id) REFERENCES answers(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_message_sender(sender) REFERENCES users(id)
                    )",
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"
                    DROP TABLE IF EXISTS 
                        users,
                        discord_linked_users,
                        discord_notification_settings,
                        form_meta_data, 
                        form_questions,
                        form_choices,
                        response_period,
                        form_webhooks,
                        answers,
                        real_answers,
                        default_answer_titles,
                        form_answer_comments,
                        form_answer_label_settings,
                        messages;
                    ",
            ))
            .await?;

        Ok(())
    }
}
