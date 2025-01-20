pub mod form;
pub mod notification_repository;
pub mod search_repository;
pub mod user_repository;

pub trait Repositories: Send + Sync {
    type ConcreteFormRepository: form::form_repository::FormRepository;
    type ConcreteFormAnswerRepository: form::answer_repository::AnswerRepository;
    type ConcreteAnswerLabelRepository: form::answer_label_repository::AnswerLabelRepository;
    type ConcreteFormQuestionRepository: form::question_repository::QuestionRepository;
    type ConcreteFormMessageRepository: form::message_repository::MessageRepository;
    type ConcreteFormCommentRepository: form::comment_repository::CommentRepository;
    type ConcreteFormLabelRepository: form::form_label_repository::FormLabelRepository;
    type ConcreteUserRepository: user_repository::UserRepository;
    type ConcreteSearchRepository: search_repository::SearchRepository;
    type ConcreteNotificationRepository: notification_repository::NotificationRepository;

    fn form_repository(&self) -> &Self::ConcreteFormRepository;
    fn form_answer_repository(&self) -> &Self::ConcreteFormAnswerRepository;
    fn answer_label_repository(&self) -> &Self::ConcreteAnswerLabelRepository;
    fn form_question_repository(&self) -> &Self::ConcreteFormQuestionRepository;
    fn form_message_repository(&self) -> &Self::ConcreteFormMessageRepository;
    fn form_comment_repository(&self) -> &Self::ConcreteFormCommentRepository;
    fn form_label_repository(&self) -> &Self::ConcreteFormLabelRepository;
    fn user_repository(&self) -> &Self::ConcreteUserRepository;
    fn search_repository(&self) -> &Self::ConcreteSearchRepository;
    fn notification_repository(&self) -> &Self::ConcreteNotificationRepository;
}
