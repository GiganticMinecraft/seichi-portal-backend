use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum UseCaseError {
    #[error("Form answer out of period.")]
    FormAnswerOutOfPeriod,
    #[error("Do not have permission to post form comment.")]
    DoNotHavePermissionToPostFormComment,
}
