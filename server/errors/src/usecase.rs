use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum UseCaseError {
    #[error("Out of period.")]
    OutOfPeriod,
    #[error("Answer not found.")]
    AnswerNotFound,
    #[error("Comment not found.")]
    CommentNotFound,
    #[error("Form not found.")]
    FormNotFound,
    #[error("Message not found.")]
    MessageNotFound,
    #[error("Notification not found.")]
    NotificationNotFound,
    #[error("Label not found.")]
    LabelNotFound,
}
