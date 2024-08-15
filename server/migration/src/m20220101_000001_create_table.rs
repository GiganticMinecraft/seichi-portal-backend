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
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    uuid CHAR(36) NOT NULL UNIQUE KEY,
                    name VARCHAR(16) NOT NULL,
                    role ENUM('ADMINISTRATOR', 'STANDARD_USER') NOT NULL
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_meta_data(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL,
                    visibility ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PRIVATE',
                    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    created_by INT NOT NULL,
                    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_by INT NOT NULL,
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
                    form_id INT NOT NULL,
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
                    form_id INT NOT NULL,
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
                    form_id INT NOT NULL,
                    url TEXT NOT NULL,
                    FOREIGN KEY fk_form_webhooks_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS answers(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    form_id INT NOT NULL,
                    user INT NOT NULL,
                    title TEXT,
                    time_stamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY fk_answers_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_answers_user(user) REFERENCES users(id)
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS real_answers(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    answer_id INT NOT NULL,
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
                    form_id INT NOT NULL,
                    title TEXT,
                    FOREIGN KEY fk_default_answer_titles_form_id(form_id) REFERENCES form_meta_data(id) ON DELETE CASCADE
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS form_answer_comments(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    answer_id INT NOT NULL,
                    commented_by INT NOT NULL,
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
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    label TEXT NOT NULL
                )",
            ))
            .await?;

        connection
            .execute(Statement::from_string(
                DatabaseBackend::MySql,
                r"CREATE TABLE IF NOT EXISTS label_settings_for_form_answers(
                    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                    answer_id INT NOT NULL,
                    label_id INT NOT NULL,
                    FOREIGN KEY fk_label_settings_for_form_answers_answer_id(answer_id) REFERENCES answers(id) ON DELETE CASCADE,
                    FOREIGN KEY fk_label_settings_for_form_answers_label_id(label_id) REFERENCES label_for_form_answers(id) ON DELETE CASCADE
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
                        form_meta_data, 
                        form_questions,
                        form_choices,
                        response_period,
                        form_webhooks,
                        answers,
                        real_answers,
                        default_answer_titles,
                        form_answer_comments,
                        form_answer_labels,
                        form_answer_label_settings;
                    ",
            ))
            .await?;

        Ok(())
    }
}
