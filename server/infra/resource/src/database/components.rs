use async_trait::async_trait;
use domain::{
    form::models::{
        AnswerId, Comment, CommentId, DefaultAnswerTitle, Form, FormAnswer, FormAnswerContent,
        FormDescription, FormId, FormTitle, Label, LabelId, Message, MessageId, OffsetAndLimit,
        Question, ResponsePeriod, Visibility, WebhookUrl,
    },
    user::models::{Role, User},
};
use errors::infra::InfraError;
use mockall::automock;
use uuid::Uuid;

use crate::dto::{
    AnswerLabelDto, CommentDto, FormAnswerContentDto, FormAnswerDto, FormDto, LabelDto, MessageDto,
    QuestionDto, SimpleFormDto,
};

#[async_trait]
pub trait DatabaseComponents: Send + Sync {
    type ConcreteFormDatabase: FormDatabase;
    type ConcreteUserDatabase: UserDatabase;
    type ConcreteSearchDatabase: SearchDatabase;
    type TransactionAcrossComponents: Send + Sync;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents>;
    fn form(&self) -> &Self::ConcreteFormDatabase;
    fn user(&self) -> &Self::ConcreteUserDatabase;
    fn search(&self) -> &Self::ConcreteSearchDatabase;
}

#[automock]
#[async_trait]
pub trait FormDatabase: Send + Sync {
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, InfraError>;
    async fn public_list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleFormDto>, InfraError>;
    async fn list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleFormDto>, InfraError>;
    async fn get(&self, form_id: FormId) -> Result<FormDto, InfraError>;
    async fn delete(&self, form_id: FormId) -> Result<(), InfraError>;
    async fn update_form_title(
        &self,
        form_id: &FormId,
        form_title: &FormTitle,
    ) -> Result<(), InfraError>;
    async fn update_form_description(
        &self,
        form_id: &FormId,
        form_description: &FormDescription,
    ) -> Result<(), InfraError>;
    async fn update_form_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), InfraError>;
    async fn update_form_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), InfraError>;
    async fn update_form_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), InfraError>;
    async fn update_form_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), InfraError>;
    async fn update_form_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), InfraError>;
    async fn post_answer(
        &self,
        user: &User,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), InfraError>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswerDto>, InfraError>;
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContentDto>, InfraError>;
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormAnswerDto>, InfraError>;
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerDto>, InfraError>;
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), InfraError>;
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
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentDto>, InfraError>;
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), InfraError>;
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), InfraError>;
    async fn create_label_for_answers(&self, label_name: String) -> Result<(), InfraError>;
    async fn get_labels_for_answers(&self) -> Result<Vec<LabelDto>, InfraError>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabelDto>, InfraError>;
    async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), InfraError>;
    async fn edit_label_for_answers(&self, label: &Label) -> Result<(), InfraError>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), InfraError>;
    async fn create_label_for_forms(&self, label_name: String) -> Result<(), InfraError>;
    async fn get_labels_for_forms(&self) -> Result<Vec<LabelDto>, InfraError>;
    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), InfraError>;
    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), InfraError>;
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), InfraError>;
    async fn post_message(&self, message: &Message) -> Result<(), InfraError>;
    async fn update_message_body(
        &self,
        message_id: MessageId,
        body: String,
    ) -> Result<(), InfraError>;
    async fn fetch_messages_answer(
        &self,
        answers: &FormAnswer,
    ) -> Result<Vec<MessageDto>, InfraError>;
    async fn fetch_message(&self, message_id: &MessageId)
        -> Result<Option<MessageDto>, InfraError>;
    async fn delete_message(&self, message_id: MessageId) -> Result<(), InfraError>;
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
}

#[automock]
#[async_trait]
pub trait SearchDatabase: Send + Sync {
    async fn search_users(&self, query: &str) -> Result<Vec<User>, InfraError>;
    async fn search_forms(&self, query: &str) -> Result<Vec<Form>, InfraError>;
    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<Label>, InfraError>;
    async fn search_labels_for_answers(&self, query: &str) -> Result<Vec<Label>, InfraError>;
    async fn search_answers(&self, query: &str) -> Result<Vec<FormAnswerContent>, InfraError>;
    async fn search_comments(
        &self,
        query: &str,
    ) -> Result<Vec<domain::search::models::Comment>, InfraError>;
}
