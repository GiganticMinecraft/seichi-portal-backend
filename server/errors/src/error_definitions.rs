use thiserror::Error;

#[derive(Error, Debug)]
pub enum FormInfraError {
    #[error("aa")]
    MutexLockFailed
}