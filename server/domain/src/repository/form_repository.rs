use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{
        Answer, AnswerId, Comment, CommentId, DefaultAnswerTitle, Form, FormDescription, FormId,
        FormQuestionUpdateSchema, FormTitle, Label, LabelId, LabelSchema, OffsetAndLimit,
        PostedAnswers, Question, ResponsePeriod, SimpleForm, Visibility, WebhookUrl,
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, Error>;
    async fn public_list(&self, offset_and_limit: OffsetAndLimit)
        -> Result<Vec<SimpleForm>, Error>;
    async fn list(&self, offset_and_limit: OffsetAndLimit) -> Result<Vec<SimpleForm>, Error>;
    async fn get(&self, id: FormId) -> Result<Form, Error>;
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
        answers: Vec<Answer>,
    ) -> Result<(), Error>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<PostedAnswers>, Error>;
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<PostedAnswers>, Error>;
    async fn get_all_answers(&self) -> Result<Vec<PostedAnswers>, Error>;
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error>;
    async fn create_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error>;
    async fn put_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error>;
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error>;
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error>;
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error>;
    async fn create_label_for_answers(&self, label: &LabelSchema) -> Result<(), Error>;
    async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error>;
    async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error>;
    async fn edit_label_for_answers(&self, label: &Label) -> Result<(), Error>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error>;
    async fn create_label_for_forms(&self, label: &LabelSchema) -> Result<(), Error>;
    async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error>;
    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error>;
    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error>;
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error>;
}
