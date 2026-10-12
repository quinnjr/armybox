//! Miscellaneous utilities
//!
//! Various utility commands that don't fit into other categories.

use super::{get_arg, has_opt};

// Modules
#[cfg(feature = "ascii")]
mod ascii;
#[cfg(feature = "base64")]
mod base64;
#[cfg(feature = "boolean")]
mod boolean;
#[cfg(feature = "cmp")]
mod cmp;
#[cfg(feature = "count")]
mod count;
#[cfg(feature = "diff")]
mod diff;
#[cfg(feature = "expr")]
mod expr;
#[cfg(feature = "factor")]
mod factor;
#[cfg(feature = "getconf")]
mod getconf;
#[cfg(feature = "getopt")]
mod getopt;
#[cfg(feature = "hash")]
mod hash;
#[cfg(feature = "help")]
mod help;
#[cfg(feature = "hexdump")]
mod hexdump;
#[cfg(feature = "iconv")]
mod iconv;
#[cfg(feature = "memeater")]
mod memeater;
#[cfg(feature = "mesg")]
mod mesg;
#[cfg(feature = "mix")]
mod mix;
#[cfg(feature = "mkpasswd")]
mod mkpasswd;
#[cfg(feature = "pwgen")]
mod pwgen;
#[cfg(feature = "readelf")]
mod readelf;
#[cfg(feature = "screen")]
mod screen;
#[cfg(feature = "terminal")]
mod terminal;
#[cfg(feature = "test")]
mod test;
#[cfg(feature = "time")]
mod time;
#[cfg(feature = "toybox")]
mod toybox;
#[cfg(feature = "ts")]
mod ts;
#[cfg(feature = "tsort")]
mod tsort;
#[cfg(feature = "unicode")]
mod unicode;
#[cfg(any(feature = "uuencode", feature = "uudecode"))]
mod uuencode;
#[cfg(any(feature = "uuidgen", feature = "mcookie"))]
mod uuidgen;
#[cfg(feature = "which")]
mod which;

// Re-export all utilities
#[cfg(feature = "ascii")]
pub use ascii::ascii;
#[cfg(feature = "base64")]
pub use base64::{base32, base64};
#[cfg(feature = "boolean")]
pub use boolean::{colon, r#false, r#true};
#[cfg(feature = "cmp")]
pub use cmp::cmp;
#[cfg(feature = "count")]
pub use count::count;
#[cfg(feature = "diff")]
pub use diff::diff;
#[cfg(feature = "expr")]
pub use expr::expr;
#[cfg(feature = "factor")]
pub use factor::factor;
#[cfg(feature = "getconf")]
pub use getconf::getconf;
#[cfg(feature = "getopt")]
pub use getopt::getopt;
#[cfg(feature = "hash")]
pub use hash::{
    cksum, crc32, md5sum, sha1sum, sha3sum, sha224sum, sha256sum, sha384sum, sha512sum,
};
#[cfg(feature = "help")]
pub use help::help;
#[cfg(feature = "hexdump")]
pub use hexdump::{hexdump, od};
#[cfg(feature = "iconv")]
pub use iconv::iconv;
#[cfg(feature = "memeater")]
pub use memeater::memeater;
#[cfg(feature = "mesg")]
pub use mesg::mesg;
#[cfg(feature = "mix")]
pub use mix::mix;
#[cfg(feature = "mkpasswd")]
pub use mkpasswd::mkpasswd;
#[cfg(feature = "pwgen")]
pub use pwgen::pwgen;
#[cfg(feature = "readelf")]
pub use readelf::readelf;
#[cfg(feature = "screen")]
pub use screen::screen;
#[cfg(feature = "terminal")]
pub use terminal::{clear, reset};
#[cfg(feature = "test")]
pub use test::{bracket, test};
#[cfg(feature = "time")]
pub use time::time;
#[cfg(feature = "toybox")]
pub use toybox::toybox;
#[cfg(feature = "ts")]
pub use ts::ts;
#[cfg(feature = "tsort")]
pub use tsort::tsort;
#[cfg(feature = "unicode")]
pub use unicode::unicode;
#[cfg(feature = "uudecode")]
pub use uuencode::uudecode;
#[cfg(feature = "uuencode")]
pub use uuencode::uuencode;
#[cfg(feature = "mcookie")]
pub use uuidgen::mcookie;
#[cfg(feature = "uuidgen")]
pub use uuidgen::uuidgen;
#[cfg(feature = "which")]
pub use which::which;
