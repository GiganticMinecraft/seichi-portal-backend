use strum_macros::Display;
use thiserror::Error;

#[derive(Display)]
pub enum Error {
    DbTransactionConstructionError,
    SqlExecutionError,
    MutexCanNotUnlock,
}

// #[derive(Error)]
// pub enum DataBaseError {
//     TransactionBeginFailed
// }
//
// impl From<DbErr>