//! File operation applets
//!
//! POSIX.1-2017 compliant file manipulation utilities.

use super::get_arg;

// Individual utility modules
#[cfg(any(
    feature = "chattr",
    feature = "lsattr",
    feature = "fstype",
    feature = "makedevs",
    feature = "setfattr"
))]
mod attrs;
#[cfg(feature = "basename")]
mod basename;
#[cfg(feature = "cat")]
mod cat;
#[cfg(feature = "cd")]
mod cd;
#[cfg(feature = "chgrp")]
mod chgrp;
#[cfg(feature = "chmod")]
mod chmod;
#[cfg(feature = "chown")]
mod chown;
#[cfg(feature = "cp")]
mod cp;
#[cfg(feature = "dd")]
mod dd;
#[cfg(feature = "dirname")]
mod dirname;
#[cfg(feature = "file")]
mod file_cmd;
#[cfg(feature = "find")]
mod find_cmd;
#[cfg(feature = "install")]
mod install;
#[cfg(feature = "link")]
mod link_cmd;
#[cfg(feature = "ln")]
mod ln;
#[cfg(feature = "ls")]
mod ls;
#[cfg(feature = "mkdir")]
mod mkdir;
#[cfg(feature = "mkfifo")]
mod mkfifo;
#[cfg(feature = "mknod")]
mod mknod;
#[cfg(feature = "mktemp")]
mod mktemp;
#[cfg(feature = "mv")]
mod mv;
#[cfg(feature = "patch")]
mod patch;
#[cfg(feature = "pwd")]
mod pwd;
#[cfg(feature = "readlink")]
mod readlink;
#[cfg(feature = "realpath")]
mod realpath;
#[cfg(feature = "rm")]
mod rm;
#[cfg(feature = "rmdir")]
mod rmdir;
#[cfg(feature = "shred")]
mod shred;
#[cfg(feature = "split")]
mod split;
#[cfg(feature = "stat")]
mod stat;
#[cfg(feature = "sync")]
mod sync_cmd;
#[cfg(feature = "touch")]
mod touch;
#[cfg(feature = "truncate")]
mod truncate;
#[cfg(feature = "unlink")]
mod unlink;
#[cfg(feature = "xargs")]
mod xargs;

// Re-export utilities
#[cfg(feature = "chattr")]
pub use attrs::chattr;
#[cfg(feature = "fstype")]
pub use attrs::fstype;
#[cfg(feature = "lsattr")]
pub use attrs::lsattr;
#[cfg(feature = "makedevs")]
pub use attrs::makedevs;
#[cfg(feature = "setfattr")]
pub use attrs::setfattr;
#[cfg(feature = "basename")]
pub use basename::basename;
#[cfg(feature = "cat")]
pub use cat::cat;
#[cfg(feature = "cd")]
pub use cd::cd;
#[cfg(feature = "chgrp")]
pub use chgrp::chgrp;
#[cfg(feature = "chmod")]
pub use chmod::chmod;
#[cfg(feature = "chown")]
pub use chown::chown;
#[cfg(feature = "cp")]
pub use cp::cp;
#[cfg(feature = "dd")]
pub use dd::dd;
#[cfg(feature = "dirname")]
pub use dirname::dirname;
#[cfg(feature = "file")]
pub use file_cmd::file;
#[cfg(feature = "find")]
pub use find_cmd::find;
#[cfg(feature = "install")]
pub use install::install;
#[cfg(feature = "link")]
pub use link_cmd::link;
#[cfg(feature = "ln")]
pub use ln::ln;
#[cfg(feature = "ls")]
pub use ls::ls;
#[cfg(feature = "mkdir")]
pub use mkdir::mkdir;
#[cfg(feature = "mkfifo")]
pub use mkfifo::mkfifo;
#[cfg(feature = "mknod")]
pub use mknod::mknod;
#[cfg(feature = "mktemp")]
pub use mktemp::mktemp;
#[cfg(feature = "mv")]
pub use mv::mv;
#[cfg(feature = "patch")]
pub use patch::patch;
#[cfg(feature = "pwd")]
pub use pwd::pwd;
#[cfg(feature = "readlink")]
pub use readlink::readlink;
#[cfg(feature = "realpath")]
pub use realpath::realpath;
#[cfg(feature = "rm")]
pub use rm::rm;
#[cfg(feature = "rmdir")]
pub use rmdir::rmdir;
#[cfg(feature = "shred")]
pub use shred::shred;
#[cfg(feature = "split")]
pub use split::split;
#[cfg(feature = "stat")]
pub use stat::stat;
#[cfg(feature = "sync")]
pub use sync_cmd::sync_cmd;
#[cfg(feature = "touch")]
pub use touch::touch;
#[cfg(feature = "truncate")]
pub use truncate::truncate;
#[cfg(feature = "unlink")]
pub use unlink::unlink;
#[cfg(feature = "xargs")]
pub use xargs::xargs;

// Re-export helpers for use by install
#[cfg(feature = "cp")]
pub(crate) use cp::copy_file;
#[cfg(feature = "mkdir")]
pub(crate) use mkdir::mkdir_parents;
