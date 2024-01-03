use async_trait::async_trait;
use domain::{
    form::models::{
        AnswerId, Comment, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle,
        FormUpdateTargets, OffsetAndLimit, PostedAnswersSchema,
    },
    user::models::{Role, User},
};
use errors::infra::InfraError;
use mockall::automock;
use uuid::Uuid;

use crate::dto::{FormDto, PostedAnswersDto, QuestionDto, SimpleFormDto};

#[async_trait]
pub trait DatabaseComponents: Send + Sync {
    type ConcreteFormDatabase: FormDatabase;
    type ConcreteUserDatabase: UserDatabase;
    type TransactionAcrossComponents: Send + Sync;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents>;
    fn form(&self) -> &Self::ConcreteFormDatabase;
    fn user(&self) -> &Self::ConcreteUserDatabase;
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
    async fn list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleFormDto>, InfraError>;
    async fn get(&self, form_id: FormId) -> Result<FormDto, InfraError>;
    async fn delete(&self, form_id: FormId) -> Result<(), InfraError>;
    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), InfraError>;
    async fn post_answer(
        &self,
        user: &User,
        answer: &PostedAnswersSchema,
    ) -> Result<(), InfraError>;
    async fn get_answers(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<PostedAnswersDto>, InfraError>;
    async fn get_all_answers(&self) -> Result<Vec<PostedAnswersDto>, InfraError>;
    async fn create_questions(&self, questions: FormQuestionUpdateSchema)
        -> Result<(), InfraError>;
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError>;
    async fn has_permission(&self, answer_id: AnswerId, user: &User) -> Result<bool, InfraError>;
    async fn post_comment(&self, comment: &Comment) -> Result<(), InfraError>;
}

#[automock]
#[async_trait]
pub trait UserDatabase: Send + Sync {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, InfraError>;
    async fn upsert_user(&self, user: &User) -> Result<(), InfraError>;
    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError>;
}
