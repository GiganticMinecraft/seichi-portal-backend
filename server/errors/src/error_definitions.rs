use strum_macros::Display;
use thiserror::Error;

#[derive(Display)]
pub enum Error {
    DbTransactionConstructionError,
    SqlExecutionError,
    MutexCanNotUnlock,
}

#[derive(Error)]
enum DataBaseError {
    #[error("line {}: Transaction begin failed.", .linenum)]
    TransactionBeginFailed
}