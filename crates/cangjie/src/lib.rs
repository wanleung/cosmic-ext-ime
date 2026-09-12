//! Cangjie (倉頡) and Quick (速成) input engine backed by libcangjie2.

mod db;
mod engine;

pub use db::{CangjieDb, CangjieError, Filter, Version};
pub use engine::{CangjieEngine, Config, Mode};
