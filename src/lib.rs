#![forbid(unsafe_code)]

pub mod map_with_file;
pub mod cfg;
pub mod file_work;
pub mod index;
pub mod map_trait;
mod file_worker;
mod tests;

pub use map_with_file::BTreeMap;
pub use map_with_file::HashMap;
pub use cfg::Cfg;
pub use cfg::Integrity;
pub use file_work::LoadFileError;
