use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum UseCaseError {
    #[error("Out of period.")]
    OutOfPeriod,
    #[error("Do not have permission to post forms comment.")]
    DoNotHavePermissionToPostFormComment,
    #[error("Answer not found.")]
    AnswerNotFound,
    #[error("Form not found.")]
    FormNotFound,
    #[error("Message not found.")]
    MessageNotFound,
    #[error("Notification not found.")]
    NotificationNotFound,
}
