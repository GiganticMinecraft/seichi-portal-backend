use crate::{
    dto::{
        AnswerLabelDto, CommentDto, DiscordUserDto, FormAnswerDto, FormDto, FormLabelDto,
        MessageDto, NotificationSettingsDto, QuestionDto,
    },
    external::discord_api::DiscordAPI,
};
use async_trait::async_trait;
use domain::search::models::{NumberOfRecordsPerAggregate, RealAnswers};
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId, AnswerLabel, AnswerLabelId},
        comment::models::{Comment, CommentId},
        message::models::{Message, MessageId},
        models::{Form, FormId, FormLabel, FormLabelId, FormLabelName},
        question::models::Question,
    },
    notification::models::NotificationPreference,
    search::models::SearchableFieldsWithOperation,
    user::models::{DiscordUser, Role, User},
};
use errors::infra::InfraError;
use mockall::automock;
use uuid::Uuid;

#[async_trait]
pub trait DatabaseComponents: Send + Sync {
    type ConcreteFormDatabase: FormDatabase;
    type ConcreteFormAnswerDatabase: FormAnswerDatabase;
    type ConcreteFormAnswerLabelDatabase: FormAnswerLabelDatabase;
    type ConcreteFormQuestionDatabase: FormQuestionDatabase;
    type ConcreteFormMessageDatabase: FormMessageDatabase;
    type ConcreteFormCommentDatabase: FormCommentDatabase;
    type ConcreteFormLabelDatabase: FormLabelDatabase;
    type ConcreteUserDatabase: UserDatabase;
    type ConcreteDiscordAPI: DiscordAPI;
    type ConcreteNotificationDatabase: NotificationDatabase;
    type ConcreteSearchDatabase: SearchDatabase;
    type TransactionAcrossComponents: Send + Sync;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents>;
    fn form(&self) -> &Self::ConcreteFormDatabase;
    fn form_answer(&self) -> &Self::ConcreteFormAnswerDatabase;
    fn form_answer_label(&self) -> &Self::ConcreteFormAnswerLabelDatabase;
    fn form_question(&self) -> &Self::ConcreteFormQuestionDatabase;
    fn form_message(&self) -> &Self::ConcreteFormMessageDatabase;
    fn form_comment(&self) -> &Self::ConcreteFormCommentDatabase;
    fn form_label(&self) -> &Self::ConcreteFormLabelDatabase;
    fn user(&self) -> &Self::ConcreteUserDatabase;
    fn discord_api(&self) -> &Self::ConcreteDiscordAPI;
    fn search(&self) -> &Self::ConcreteSearchDatabase;
    fn notification(&self) -> &Self::ConcreteNotificationDatabase;
}

#[automock]
#[async_trait]
pub trait FormDatabase: Send + Sync {
    async fn create(&self, form: &Form, user: &User) -> Result<(), InfraError>;
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<FormDto>, InfraError>;
    async fn get(&self, form_id: FormId) -> Result<Option<FormDto>, InfraError>;
    async fn delete(&self, form_id: FormId) -> Result<(), InfraError>;
    async fn update(&self, form: &Form, updated_by: &User) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormAnswerDatabase: Send + Sync {
    async fn post_answer(&self, answer: &AnswerEntry) -> Result<(), InfraError>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswerDto>, InfraError>;
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormAnswerDto>, InfraError>;
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerDto>, InfraError>;
    async fn get_answers_by_answer_ids(
        &self,
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<FormAnswerDto>, InfraError>;
    async fn update_answer_entry(&self, answer_entry: &AnswerEntry) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormAnswerLabelDatabase: Send + Sync {
    async fn create_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError>;
    async fn get_labels_for_answers(&self) -> Result<Vec<AnswerLabelDto>, InfraError>;
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AnswerLabelDto>, InfraError>;
    async fn get_labels_for_answers_by_label_ids(
        &self,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Vec<AnswerLabelDto>, InfraError>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabelDto>, InfraError>;
    async fn delete_label_for_answers(&self, label_id: AnswerLabelId) -> Result<(), InfraError>;
    async fn edit_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormQuestionDatabase: Send + Sync {
    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError>;
    async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError>;
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError>;
}

#[async_trait]
pub trait FormMessageDatabase: Send + Sync {
    async fn post_message(&self, message: &Message) -> Result<(), InfraError>;
    async fn update_message_body(
        &self,
        message_id: MessageId,
        body: String,
    ) -> Result<(), InfraError>;
    async fn fetch_messages_by_form_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<MessageDto>, InfraError>;
    async fn fetch_message(&self, message_id: &MessageId)
    -> Result<Option<MessageDto>, InfraError>;
    async fn delete_message(&self, message_id: MessageId) -> Result<(), InfraError>;
}

#[automock]
#[async_trait]
pub trait FormCommentDatabase: Send + Sync {
    async fn get_comment(&self, comment_id: CommentId) -> Result<Option<CommentDto>, InfraError>;
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentDto>, InfraError>;
    async fn get_all_comments(&self) -> Result<Vec<CommentDto>, InfraError>;
    async fn upsert_comment(
        &self,
        answer_id: AnswerId,
        comment: &Comment,
    ) -> Result<(), InfraError>;
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormLabelDatabase: Send + Sync {
    async fn create_label_for_forms(&self, label: &FormLabel) -> Result<(), InfraError>;
    async fn fetch_labels(&self) -> Result<Vec<FormLabelDto>, InfraError>;
    async fn fetch_labels_by_ids(
        &self,
        ids: Vec<FormLabelId>,
    ) -> Result<Vec<FormLabelDto>, InfraError>;
    async fn delete_label_for_forms(&self, label_id: FormLabelId) -> Result<(), InfraError>;
    async fn fetch_label(&self, id: FormLabelId) -> Result<Option<FormLabelDto>, InfraError>;
    async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        name: FormLabelName,
    ) -> Result<(), InfraError>;
    async fn fetch_labels_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormLabelDto>, InfraError>;
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait UserDatabase: Send + Sync {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, InfraError>;
    async fn upsert_user(&self, user: &User) -> Result<(), InfraError>;
    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError>;
    async fn fetch_all_users(&self) -> Result<Vec<User>, InfraError>;
    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
        expires: i32,
    ) -> Result<String, InfraError>;
    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<User>, InfraError>;
    async fn end_user_session(&self, session_id: String) -> Result<(), InfraError>;
    async fn link_discord_user(
        &self,
        discord_user: &DiscordUser,
        user: &User,
    ) -> Result<(), InfraError>;
    async fn unlink_discord_user(&self, user: &User) -> Result<(), InfraError>;
    async fn fetch_discord_user(&self, user: &User) -> Result<Option<DiscordUserDto>, InfraError>;
    async fn fetch_size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait SearchDatabase: Send + Sync {
    async fn search_users(&self, query: &str) -> Result<Vec<User>, InfraError>;
    async fn search_forms(&self, query: &str) -> Result<Vec<Form>, InfraError>;
    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<FormLabel>, InfraError>;
    async fn search_labels_for_answers(&self, query: &str) -> Result<Vec<AnswerLabel>, InfraError>;
    async fn search_answers(&self, query: &str) -> Result<Vec<RealAnswers>, InfraError>;
    async fn search_comments(&self, query: &str) -> Result<Vec<Comment>, InfraError>;
    async fn sync_search_engine(
        &self,
        data: &[SearchableFieldsWithOperation],
    ) -> Result<(), InfraError>;
    async fn search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, InfraError>;
}

#[automock]
#[async_trait]
pub trait NotificationDatabase: Send + Sync {
    async fn upsert_notification_settings(
        &self,
        notification_settings: &NotificationPreference,
    ) -> Result<(), InfraError>;
    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<NotificationSettingsDto>, InfraError>;
}
