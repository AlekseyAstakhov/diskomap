#![forbid(unsafe_code)]

pub mod map_with_file;
pub mod cfg;
pub mod format;
pub mod index;
pub mod map_trait;
pub mod bin_format;
pub mod text_format;
mod file_worker;
mod tests;

pub use map_with_file::BTreeMap;
pub use map_with_file::HashMap;
pub use cfg::Cfg;
pub use cfg::Format;
pub use cfg::Integrity;
pub use format::LoadFileError;
