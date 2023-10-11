use std::fmt::{Debug, Display, Formatter};

use cqrs_es::persist::PersistenceError;
use cqrs_es::AggregateError;
use sqlx::Error;

#[derive(Debug)]
pub enum SqliteAggregateError {
    OptimisticLock,
    DeserializationError(Box<dyn std::error::Error + Send + Sync + 'static>),
    UnknownError(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Display for SqliteAggregateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SqliteAggregateError::OptimisticLock => write!(f, "optimistic lock error"),
            SqliteAggregateError::UnknownError(error) => write!(f, "{}", error),
            SqliteAggregateError::DeserializationError(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for SqliteAggregateError {}

impl From<sqlx::Error> for SqliteAggregateError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            Error::Database(database_error) => {
                if let Some(code) = database_error.code() {
                    if code.as_ref() == "1555" {
                        // UNIQUE constraint failed
                        return SqliteAggregateError::OptimisticLock;
                    }
                }
                SqliteAggregateError::UnknownError(Box::new(err))
            }
            _ => SqliteAggregateError::UnknownError(Box::new(err)),
        }
    }
}

impl<T: std::error::Error> From<SqliteAggregateError> for AggregateError<T> {
    fn from(err: SqliteAggregateError) -> Self {
        match err {
            SqliteAggregateError::OptimisticLock => AggregateError::AggregateConflict,
            SqliteAggregateError::DeserializationError(error) => {
                AggregateError::DeserializationError(error)
            }
            SqliteAggregateError::UnknownError(error) => AggregateError::UnexpectedError(error),
        }
    }
}

impl From<serde_json::Error> for SqliteAggregateError {
    fn from(err: serde_json::Error) -> Self {
        match err.classify() {
            serde_json::error::Category::Data | serde_json::error::Category::Syntax => {
                SqliteAggregateError::DeserializationError(Box::new(err))
            }
            serde_json::error::Category::Io | serde_json::error::Category::Eof => {
                SqliteAggregateError::UnknownError(Box::new(err))
            }
        }
    }
}

impl From<SqliteAggregateError> for PersistenceError {
    fn from(err: SqliteAggregateError) -> Self {
        match err {
            SqliteAggregateError::OptimisticLock => PersistenceError::OptimisticLockError,
            SqliteAggregateError::DeserializationError(error) => {
                PersistenceError::UnknownError(error)
            }
            SqliteAggregateError::UnknownError(error) => PersistenceError::UnknownError(error),
        }
    }
}
