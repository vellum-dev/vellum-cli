mod compat;
mod exec;
mod index;
mod package;
mod version;

pub use compat::check_os_compatibility;
pub use exec::Apk;
pub use index::{fetch_remote_index, parse_index_tar_gz, Package};
pub use package::{generate_device_package, generate_remarkable_os_package};
