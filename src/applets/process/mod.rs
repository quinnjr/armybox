//! Process utilities for ArmyBox
//!
//! Process management, monitoring, and control utilities.

use super::get_arg;

// Process management and monitoring
#[cfg(feature = "iotop")]
mod iotop;
#[cfg(feature = "kill")]
mod kill;
#[cfg(feature = "killall")]
mod killall;
#[cfg(feature = "killall5")]
mod killall5;
#[cfg(feature = "pgrep")]
mod pgrep;
#[cfg(feature = "pidof")]
mod pidof;
#[cfg(feature = "pkill")]
mod pkill;
#[cfg(feature = "pmap")]
mod pmap;
#[cfg(feature = "ps")]
mod ps;
#[cfg(feature = "pwdx")]
mod pwdx;
#[cfg(feature = "top")]
mod top;

// Process execution control
#[cfg(feature = "chrt")]
mod chrt;
#[cfg(feature = "ionice")]
mod ionice;
#[cfg(feature = "iorenice")]
mod iorenice;
#[cfg(feature = "nice")]
mod nice;
#[cfg(feature = "nohup")]
mod nohup;
#[cfg(feature = "prlimit")]
mod prlimit;
#[cfg(feature = "renice")]
mod renice;
#[cfg(feature = "setsid")]
mod setsid;
#[cfg(feature = "taskset")]
mod taskset;
#[cfg(feature = "timeout")]
mod timeout;
#[cfg(feature = "uclampset")]
mod uclampset;

// Namespace utilities
#[cfg(feature = "nsenter")]
mod nsenter;
#[cfg(feature = "unshare")]
mod unshare;

// Re-export process management utilities
#[cfg(feature = "iotop")]
pub use iotop::iotop;
#[cfg(feature = "kill")]
pub use kill::kill;
#[cfg(feature = "killall")]
pub use killall::killall;
#[cfg(feature = "killall5")]
pub use killall5::killall5;
#[cfg(feature = "pgrep")]
pub use pgrep::pgrep;
#[cfg(feature = "pidof")]
pub use pidof::pidof;
#[cfg(feature = "pkill")]
pub use pkill::pkill;
#[cfg(feature = "pmap")]
pub use pmap::pmap;
#[cfg(feature = "ps")]
pub use ps::ps;
#[cfg(feature = "pwdx")]
pub use pwdx::pwdx;
#[cfg(feature = "top")]
pub use top::top;

// Re-export process execution control utilities
#[cfg(feature = "chrt")]
pub use chrt::chrt;
#[cfg(feature = "ionice")]
pub use ionice::ionice;
#[cfg(feature = "iorenice")]
pub use iorenice::iorenice;
#[cfg(feature = "nice")]
pub use nice::nice;
#[cfg(feature = "nohup")]
pub use nohup::nohup;
#[cfg(feature = "prlimit")]
pub use prlimit::prlimit;
#[cfg(feature = "renice")]
pub use renice::renice;
#[cfg(feature = "setsid")]
pub use setsid::setsid;
#[cfg(feature = "taskset")]
pub use taskset::taskset;
#[cfg(feature = "timeout")]
pub use timeout::timeout;
#[cfg(feature = "uclampset")]
pub use uclampset::uclampset;

// Re-export namespace utilities
#[cfg(feature = "nsenter")]
pub use nsenter::nsenter;
#[cfg(feature = "unshare")]
pub use unshare::unshare;
