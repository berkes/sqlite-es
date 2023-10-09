//! # sqlite-es
//!
//! > A Sqlite implementation of the `EventStore` trait in [cqrs-es](https://crates.io/crates/cqrs-es).
//!
pub use crate::cqrs::*;
pub use crate::error::*;
pub use crate::event_repository::*;

mod cqrs;
mod error;
mod event_repository;
