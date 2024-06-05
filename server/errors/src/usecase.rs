use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum UseCaseError {
    #[error("Out of period.")]
    OutOfPeriod,
    #[error("Do not have permission to post form comment.")]
    DoNotHavePermissionToPostFormComment,
}
