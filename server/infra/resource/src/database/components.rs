use crate::{
    external::discord_api::DiscordAPI,
    records::{
        ActiveFormRecord, AnswerLabelRecord, ArchivedFormRecord, CommentHistoryRecord,
        CommentRecord, DiscordUserRecord, FormAnswerRecord, FormLabelRecord, MessageHistoryRecord,
        MessageRecord, NotificationSettingsRecord,
    },
};
use async_trait::async_trait;
use domain::search::models::{
    AnswerLabelSearchHit, AnswerSearchHit, CommentSearchHit, FormLabelSearchHit, FormSearchHit,
    NumberOfRecordsPerAggregate, UserSearchHit,
};
use domain::{
    account::models::{
        AccountUser, DiscordAccountLink, Role, UserGroup, UserGroupId, UserPagePosition,
    },
    form::{
        answer::{AnswerEntry, AnswerId, AnswerLabel, AnswerLabelId, AnswerSubmitterRestriction},
        comment::{Comment, CommentHistoryPagePosition, CommentId},
        message::{Message, MessageHistoryPagePosition, MessageId},
        models::{
            ActiveForm, ArchivedForm, ArchivedFormPagePosition, FormId, FormLabel, FormLabelId,
            FormLabelName, FormPagePosition,
        },
    },
    notification::models::NotificationPreference,
    pagination::{Page, PageRequest},
    search::models::SearchableFieldsWithOperation,
};
use errors::infra::InfraError;
use mockall::automock;
use uuid::Uuid;

#[async_trait]
pub trait DatabaseComponents: Send + Sync {
    type ConcreteFormDatabase: FormDatabase;
    type ConcreteFormAnswerDatabase: FormAnswerDatabase;
    type ConcreteFormAnswerLabelDatabase: FormAnswerLabelDatabase;
    type ConcreteFormMessageDatabase: FormMessageDatabase;
    type ConcreteFormMessageThreadDatabase: FormMessageThreadDatabase;
    type ConcreteFormCommentDatabase: FormCommentDatabase;
    type ConcreteFormLabelDatabase: FormLabelDatabase;
    type ConcreteAnswerSubmitterRestrictionDatabase: AnswerSubmitterRestrictionDatabase;
    type ConcreteUserDatabase: UserDatabase;
    type ConcreteDiscordAPI: DiscordAPI;
    type ConcreteNotificationDatabase: NotificationDatabase;
    type ConcreteSearchDatabase: SearchDatabase;
    type TransactionAcrossComponents: Send + Sync;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents>;
    fn form(&self) -> &Self::ConcreteFormDatabase;
    fn form_answer(&self) -> &Self::ConcreteFormAnswerDatabase;
    fn form_answer_label(&self) -> &Self::ConcreteFormAnswerLabelDatabase;
    fn form_message(&self) -> &Self::ConcreteFormMessageDatabase;
    fn form_message_thread(&self) -> &Self::ConcreteFormMessageThreadDatabase;
    fn form_comment(&self) -> &Self::ConcreteFormCommentDatabase;
    fn form_label(&self) -> &Self::ConcreteFormLabelDatabase;
    fn answer_submitter_restriction(&self) -> &Self::ConcreteAnswerSubmitterRestrictionDatabase;
    fn user(&self) -> &Self::ConcreteUserDatabase;
    fn discord_api(&self) -> &Self::ConcreteDiscordAPI;
    fn search(&self) -> &Self::ConcreteSearchDatabase;
    fn notification(&self) -> &Self::ConcreteNotificationDatabase;
}

#[automock]
#[async_trait]
pub trait FormDatabase: Send + Sync {
    async fn create(&self, form: &ActiveForm, user: &AccountUser) -> Result<(), InfraError>;
    async fn list(
        &self,
        request: PageRequest<FormPagePosition>,
    ) -> Result<Page<ActiveFormRecord, FormPagePosition>, InfraError>;
    async fn list_all(&self) -> Result<Vec<ActiveFormRecord>, InfraError>;
    async fn get(&self, form_id: FormId) -> Result<Option<ActiveFormRecord>, InfraError>;
    async fn list_archived(
        &self,
        request: PageRequest<ArchivedFormPagePosition>,
        query: Option<String>,
    ) -> Result<Page<ArchivedFormRecord, ArchivedFormPagePosition>, InfraError>;
    async fn get_archived(&self, form_id: FormId)
    -> Result<Option<ArchivedFormRecord>, InfraError>;
    async fn archive(&self, form: &ArchivedForm) -> Result<ArchivedForm, InfraError>;
    async fn restore(&self, form_id: FormId) -> Result<(), InfraError>;
    async fn update(&self, form: &ActiveForm, updated_by: &AccountUser) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
    async fn list_answer_entries(
        &self,
        form_id: FormId,
        request: PageRequest<domain::form::answer::AnswerPagePosition>,
    ) -> Result<Page<AnswerEntry, domain::form::answer::AnswerPagePosition>, InfraError>;
    async fn list_all_answer_entries(
        &self,
        request: PageRequest<domain::form::answer::AnswerPagePosition>,
    ) -> Result<Page<AnswerEntry, domain::form::answer::AnswerPagePosition>, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormAnswerDatabase: Send + Sync {
    async fn post_answer(&self, answer: &AnswerEntry, form_id: FormId) -> Result<(), InfraError>;
    async fn get_answers(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<FormAnswerRecord>, InfraError>;
    async fn get_answers_by_answer_ids(
        &self,
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<FormAnswerRecord>, InfraError>;
    async fn update_answer_entry(
        &self,
        answer_entry: &AnswerEntry,
        form_id: FormId,
    ) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormAnswerLabelDatabase: Send + Sync {
    async fn create_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError>;
    async fn get_labels_for_answers(&self) -> Result<Vec<AnswerLabelRecord>, InfraError>;
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AnswerLabelRecord>, InfraError>;
    async fn get_labels_for_answers_by_label_ids(
        &self,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Vec<AnswerLabelRecord>, InfraError>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabelRecord>, InfraError>;
    async fn delete_label_for_answers(&self, label_id: AnswerLabelId) -> Result<(), InfraError>;
    async fn edit_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[async_trait]
pub trait FormMessageDatabase: Send + Sync {
    async fn post_message(&self, message: &Message, answer_id: AnswerId) -> Result<(), InfraError>;
    async fn update_message_with_history(
        &self,
        message: &Message,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError>;
    async fn fetch_messages_by_form_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<MessageRecord>, InfraError>;
    async fn fetch_messages_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<MessageRecord>, InfraError>;
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<MessageRecord>, InfraError>;
    async fn delete_message_with_history(
        &self,
        message_id: MessageId,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError>;
    async fn fetch_history(
        &self,
        answer_id: AnswerId,
        request: PageRequest<MessageHistoryPagePosition>,
    ) -> Result<Page<MessageHistoryRecord, MessageHistoryPagePosition>, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormMessageThreadDatabase: Send + Sync {
    async fn create_message_thread(
        &self,
        answer_id: &str,
        answer_author_id: &str,
    ) -> Result<(), InfraError>;
    async fn get_thread_author_by_answer_id(
        &self,
        answer_id: &str,
    ) -> Result<Option<String>, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormCommentDatabase: Send + Sync {
    async fn get_comment(&self, comment_id: CommentId)
    -> Result<Option<CommentRecord>, InfraError>;
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentRecord>, InfraError>;
    async fn get_all_comments(&self) -> Result<Vec<CommentRecord>, InfraError>;
    async fn create_comment(&self, comment: &Comment) -> Result<(), InfraError>;
    async fn update_comment_with_history(
        &self,
        comment: &Comment,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError>;
    async fn delete_comment_with_history(
        &self,
        comment_id: CommentId,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError>;
    async fn get_history(
        &self,
        answer_id: AnswerId,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<CommentHistoryRecord, CommentHistoryPagePosition>, InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait FormLabelDatabase: Send + Sync {
    async fn create_label_for_forms(&self, label: &FormLabel) -> Result<(), InfraError>;
    async fn fetch_labels(&self) -> Result<Vec<FormLabelRecord>, InfraError>;
    async fn fetch_labels_by_ids(
        &self,
        ids: Vec<FormLabelId>,
    ) -> Result<Vec<FormLabelRecord>, InfraError>;
    async fn delete_label_for_forms(&self, label_id: FormLabelId) -> Result<(), InfraError>;
    async fn fetch_label(&self, id: FormLabelId) -> Result<Option<FormLabelRecord>, InfraError>;
    async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        name: FormLabelName,
    ) -> Result<(), InfraError>;
    async fn fetch_labels_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormLabelRecord>, InfraError>;
    async fn size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait UserDatabase: Send + Sync {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<AccountUser>, InfraError>;
    async fn find_by_ids(&self, uuids: Vec<Uuid>) -> Result<Vec<AccountUser>, InfraError>;
    async fn upsert_user(&self, user: &AccountUser) -> Result<(), InfraError>;
    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError>;
    async fn create_user_group(&self, group: &UserGroup) -> Result<(), InfraError>;
    async fn update_user_group(&self, group: &UserGroup) -> Result<(), InfraError>;
    async fn delete_user_group(&self, group_id: UserGroupId) -> Result<(), InfraError>;
    async fn find_user_group(&self, group_id: UserGroupId)
    -> Result<Option<UserGroup>, InfraError>;
    async fn fetch_user_groups(&self) -> Result<Vec<UserGroup>, InfraError>;
    async fn fetch_users_by_group(
        &self,
        group_id: UserGroupId,
    ) -> Result<Vec<AccountUser>, InfraError>;
    async fn add_user_to_group(
        &self,
        group_id: UserGroupId,
        user_id: Uuid,
    ) -> Result<(), InfraError>;
    async fn remove_user_from_group(
        &self,
        group_id: UserGroupId,
        user_id: Uuid,
    ) -> Result<(), InfraError>;
    async fn fetch_all_users(&self) -> Result<Vec<AccountUser>, InfraError>;
    async fn fetch_users_page(
        &self,
        request: PageRequest<UserPagePosition>,
    ) -> Result<Page<AccountUser, UserPagePosition>, InfraError>;
    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &AccountUser,
        expires: u32,
    ) -> Result<String, InfraError>;
    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<AccountUser>, InfraError>;
    async fn end_user_session(&self, session_id: String) -> Result<(), InfraError>;
    async fn link_discord_user(&self, link: &DiscordAccountLink) -> Result<(), InfraError>;
    async fn unlink_discord_user(&self, link: &DiscordAccountLink) -> Result<(), InfraError>;
    async fn fetch_discord_user(
        &self,
        user: &AccountUser,
    ) -> Result<Option<DiscordUserRecord>, InfraError>;
    async fn fetch_size(&self) -> Result<u32, InfraError>;
}

#[automock]
#[async_trait]
pub trait AnswerSubmitterRestrictionDatabase: Send + Sync {
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AnswerSubmitterRestriction>, InfraError>;
    async fn list_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Vec<AnswerSubmitterRestriction>, InfraError>;
    async fn restrict(&self, restriction: &AnswerSubmitterRestriction) -> Result<(), InfraError>;
    async fn lift(&self, submitter_id: Uuid, lifted_by: Uuid) -> Result<(), InfraError>;
}

#[automock]
#[async_trait]
pub trait SearchDatabase: Send + Sync {
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, InfraError>;
    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, InfraError>;
    async fn search_labels_for_forms(
        &self,
        query: &str,
    ) -> Result<Vec<FormLabelSearchHit>, InfraError>;
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, InfraError>;
    async fn search_answers(&self, query: &str) -> Result<Vec<AnswerSearchHit>, InfraError>;
    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, InfraError>;
    async fn sync_search_engine(
        &self,
        data: &[SearchableFieldsWithOperation],
    ) -> Result<(), InfraError>;
    async fn search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, InfraError>;
    async fn initialize_search_engine(&self) -> Result<(), InfraError>;
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
    ) -> Result<Option<NotificationSettingsRecord>, InfraError>;
}
