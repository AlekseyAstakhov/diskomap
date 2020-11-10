#![forbid(unsafe_code)]

pub mod btree;
pub mod cfg;
pub mod file_work;
pub mod index;
pub mod map_trait;
mod file_worker;
mod tests;

pub use btree::BTree;
pub use cfg::Integrity;
pub use file_work::LoadFileError;
