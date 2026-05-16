pub mod form;
pub mod health_check_repository;
pub mod notification_repository;
pub mod search_repository;
pub mod user_repository;

pub trait Repositories: Send + Sync {
    type ConcreteActiveFormRepository: form::active_form_repository::ActiveFormRepository;
    type ConcreteArchivedFormRepository: form::archived_form_repository::ArchivedFormRepository;
    type ConcreteFormAnswerRepository: form::answer_repository::AnswerRepository;
    type ConcreteAnswerLabelRepository: form::answer_label_repository::AnswerLabelRepository;
    type ConcreteFormMessageRepository: form::message_repository::MessageRepository;
    type ConcreteFormCommentRepository: form::comment_repository::CommentRepository;
    type ConcreteFormLabelRepository: form::form_label_repository::FormLabelRepository;
    type ConcreteUserRepository: user_repository::UserRepository;
    type ConcreteSearchRepository: search_repository::SearchRepository;
    type ConcreteNotificationRepository: notification_repository::NotificationRepository;
    type ConcreteHealthCheckRepository: health_check_repository::HealthCheckRepository;
    fn active_form_repository(&self) -> &Self::ConcreteActiveFormRepository;
    fn archived_form_repository(&self) -> &Self::ConcreteArchivedFormRepository;
    fn form_answer_repository(&self) -> &Self::ConcreteFormAnswerRepository;
    fn answer_label_repository(&self) -> &Self::ConcreteAnswerLabelRepository;
    fn form_message_repository(&self) -> &Self::ConcreteFormMessageRepository;
    fn form_comment_repository(&self) -> &Self::ConcreteFormCommentRepository;
    fn form_label_repository(&self) -> &Self::ConcreteFormLabelRepository;
    fn user_repository(&self) -> &Self::ConcreteUserRepository;
    fn search_repository(&self) -> &Self::ConcreteSearchRepository;
    fn notification_repository(&self) -> &Self::ConcreteNotificationRepository;
    fn health_check_repository(&self) -> &Self::ConcreteHealthCheckRepository;
}
