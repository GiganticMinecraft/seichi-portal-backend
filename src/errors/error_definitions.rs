use strum_macros::Display;

#[derive(Display)]
pub enum Error {
    DbTransactionConstructionError,
    SqlExecutionError,
    MutexCanNotUnlock,
}
