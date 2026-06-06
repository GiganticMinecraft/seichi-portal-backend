pub mod form;
pub mod health_check_repository;
pub mod notification_repository;
pub mod search_repository;
pub mod user_repository;

pub trait Repositories: Send + Sync {
    type ConcreteActiveFormRepository: form::active_form_repository::ActiveFormRepository;
    type ConcreteArchivedFormRepository: form::archived_form_repository::ArchivedFormRepository;
    type ConcreteAnswerEntryRepository: form::answer_entry_repository::AnswerEntryRepository;
    type ConcreteAnswerLabelRepository: form::answer_label_repository::AnswerLabelRepository;
    type ConcreteCommentRepository: form::comment_repository::CommentRepository;
    type ConcreteMessageThreadRepository: form::message_thread_repository::MessageThreadRepository;
    type ConcreteFormLabelRepository: form::form_label_repository::FormLabelRepository;
    type ConcreteUserRepository: user_repository::UserRepository;
    type ConcreteSearchRepository: search_repository::SearchRepository;
    type ConcreteNotificationRepository: notification_repository::NotificationRepository;
    type ConcreteHealthCheckRepository: health_check_repository::HealthCheckRepository;
    fn active_form_repository(&self) -> &Self::ConcreteActiveFormRepository;
    fn archived_form_repository(&self) -> &Self::ConcreteArchivedFormRepository;
    fn answer_entry_repository(&self) -> &Self::ConcreteAnswerEntryRepository;
    fn answer_label_repository(&self) -> &Self::ConcreteAnswerLabelRepository;
    fn comment_repository(&self) -> &Self::ConcreteCommentRepository;
    fn message_thread_repository(&self) -> &Self::ConcreteMessageThreadRepository;
    fn form_label_repository(&self) -> &Self::ConcreteFormLabelRepository;
    fn user_repository(&self) -> &Self::ConcreteUserRepository;
    fn search_repository(&self) -> &Self::ConcreteSearchRepository;
    fn notification_repository(&self) -> &Self::ConcreteNotificationRepository;
    fn health_check_repository(&self) -> &Self::ConcreteHealthCheckRepository;
}
