use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::{AnswerId, AnswerLabel, FormAnswer, FormAnswerContent},
        comment::models::{Comment, CommentId},
        message::models::{Message, MessageId},
        models::{
            DefaultAnswerTitle, Form, FormDescription, FormId, FormTitle, Label, LabelId,
            ResponsePeriod, Visibility, WebhookUrl,
        },
        question::models::Question,
    },
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Read, Update},
    user::models::User,
};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(&self, form: &Form, user: &User) -> Result<(), Error>;
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<AuthorizationGuard<Form, Read>>, Error>;
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<Form, Read>>, Error>;
    async fn delete(&self, id: FormId) -> Result<(), Error>;
    async fn update_title(&self, form_id: &FormId, title: &FormTitle) -> Result<(), Error>;
    async fn update_description(
        &self,
        form_id: &FormId,
        description: &FormDescription,
    ) -> Result<(), Error>;
    async fn update_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), Error>;
    async fn update_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), Error>;
    async fn update_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), Error>;
    async fn update_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error>;
    async fn update_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error>;
    async fn post_answer(
        &self,
        user: &User,
        form_id: FormId,
        title: DefaultAnswerTitle,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswer>, Error>;
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContent>, Error>;
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<FormAnswer>, Error>;
    async fn get_all_answers(&self) -> Result<Vec<FormAnswer>, Error>;
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error>;
    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error>;
    async fn put_questions(&self, form_id: FormId, questions: Vec<Question>) -> Result<(), Error>;
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error>;
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error>;
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error>;
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error>;
    async fn create_label_for_answers(&self, label_name: String) -> Result<(), Error>;
    async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabel>, Error>;
    async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error>;
    async fn edit_label_for_answers(&self, label: &Label) -> Result<(), Error>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error>;
    async fn create_label_for_forms(&self, name: String) -> Result<(), Error>;
    async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error>;
    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error>;
    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error>;
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error>;
    async fn post_message(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Create>,
    ) -> Result<(), Error>;
    async fn fetch_messages_by_answer(
        &self,
        answers: &FormAnswer,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error>;
    async fn update_message_body(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Update>,
        body: String,
    ) -> Result<(), Error>;
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<AuthorizationGuard<Message, Read>>, Error>;
    async fn delete_message(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Delete>,
    ) -> Result<(), Error>;
}
