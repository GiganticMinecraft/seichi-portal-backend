use thiserror::Error;

#[derive(Error, Debug)]
pub enum FormInfraError {
    #[error("[FormInfra] Began Mutex lock failed.")]
    MutexLockFailed,
}
