mod add;
mod check_os;
mod del;
mod reenable;
mod self_uninstall;
mod testing;
mod upgrade;

pub use add::handle_add;
pub use check_os::handle_check_os;
pub use del::{handle_del, handle_purge};
pub use reenable::handle_reenable;
pub use self_uninstall::handle_self_uninstall;
pub use testing::handle_testing;
pub use upgrade::handle_upgrade;
