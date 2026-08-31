pub mod form;
pub mod form_submission_restriction_repository;
pub mod global_discord_webhook_repository;
pub mod health_check_repository;
pub mod minecraft_ban_repository;
pub mod notification_repository;
pub mod redmine_import_repository;
pub mod search_repository;
pub mod user_repository;

pub trait Repositories: Send + Sync {
    type ConcreteActiveFormRepository: form::active_form_repository::ActiveFormRepository;
    type ConcreteArchivedFormRepository: form::archived_form_repository::ArchivedFormRepository;
    type ConcreteAnswerEntryRepository: form::answer_entry_repository::AnswerEntryRepository;
    type ConcreteAnswerRelationRepository: form::answer_relation_repository::AnswerRelationRepository;
    type ConcreteAnswerLabelRepository: form::answer_label_repository::AnswerLabelRepository;
    type ConcreteCommentThreadRepository: form::comment_thread_repository::CommentThreadRepository;
    type ConcreteMessageThreadRepository: form::message_thread_repository::MessageThreadRepository;
    type ConcreteFormLabelRepository: form::form_label_repository::FormLabelRepository;
    type ConcreteFormSubmissionRestrictionRepository: form_submission_restriction_repository::FormSubmissionRestrictionRepository;
    type ConcreteUserRepository: user_repository::UserRepository;
    type ConcreteSearchRepository: search_repository::SearchRepository;
    type ConcreteNotificationRepository: notification_repository::NotificationRepository;
    type ConcreteHealthCheckRepository: health_check_repository::HealthCheckRepository;
    type ConcreteMinecraftBanRepository: minecraft_ban_repository::MinecraftBanRepository;
    fn active_form_repository(&self) -> &Self::ConcreteActiveFormRepository;
    fn archived_form_repository(&self) -> &Self::ConcreteArchivedFormRepository;
    fn answer_entry_repository(&self) -> &Self::ConcreteAnswerEntryRepository;
    fn answer_relation_repository(&self) -> &Self::ConcreteAnswerRelationRepository;
    fn answer_label_repository(&self) -> &Self::ConcreteAnswerLabelRepository;
    fn comment_thread_repository(&self) -> &Self::ConcreteCommentThreadRepository;
    fn message_thread_repository(&self) -> &Self::ConcreteMessageThreadRepository;
    fn form_label_repository(&self) -> &Self::ConcreteFormLabelRepository;
    fn form_submission_restriction_repository(
        &self,
    ) -> &Self::ConcreteFormSubmissionRestrictionRepository;
    fn user_repository(&self) -> &Self::ConcreteUserRepository;
    fn search_repository(&self) -> &Self::ConcreteSearchRepository;
    fn notification_repository(&self) -> &Self::ConcreteNotificationRepository;
    fn health_check_repository(&self) -> &Self::ConcreteHealthCheckRepository;
    fn minecraft_ban_repository(&self) -> &Self::ConcreteMinecraftBanRepository;
}
