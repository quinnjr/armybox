//! Text processing applets
//!
//! POSIX.1-2017 compliant text manipulation utilities.

// Individual utility modules
#[cfg(feature = "awk")]
mod awk;
#[cfg(feature = "comm")]
mod comm;
#[cfg(feature = "cut")]
mod cut;
#[cfg(feature = "dos2unix")]
mod dos2unix;
#[cfg(feature = "echo")]
mod echo;
#[cfg(feature = "expand")]
mod expand;
#[cfg(feature = "fmt")]
mod fmt;
#[cfg(feature = "fold")]
mod fold;
#[cfg(feature = "grep")]
mod grep;
#[cfg(feature = "head")]
mod head;
#[cfg(feature = "nl")]
mod nl;
#[cfg(feature = "paste")]
mod paste;
#[cfg(feature = "printf")]
mod printf;
#[cfg(feature = "rev")]
mod rev;
#[cfg(feature = "sed")]
mod sed;
#[cfg(feature = "seq")]
mod seq;
#[cfg(feature = "sort")]
mod sort;
#[cfg(feature = "strings")]
mod strings;
#[cfg(feature = "tac")]
mod tac;
#[cfg(feature = "tail")]
mod tail;
#[cfg(feature = "tee")]
mod tee;
#[cfg(feature = "tr")]
mod tr;
#[cfg(feature = "unexpand")]
mod unexpand;
#[cfg(feature = "uniq")]
mod uniq;
#[cfg(feature = "unix2dos")]
mod unix2dos;
#[cfg(feature = "wc")]
mod wc;
#[cfg(feature = "yes")]
mod yes;

// Re-export utilities
#[cfg(feature = "awk")]
pub use awk::awk;
#[cfg(feature = "comm")]
pub use comm::comm;
#[cfg(feature = "cut")]
pub use cut::cut;
#[cfg(feature = "dos2unix")]
pub use dos2unix::dos2unix;
#[cfg(feature = "echo")]
pub use echo::echo;
#[cfg(feature = "expand")]
pub use expand::expand;
#[cfg(feature = "fmt")]
pub use fmt::fmt;
#[cfg(feature = "fold")]
pub use fold::fold;
#[cfg(feature = "grep")]
pub use grep::{egrep, fgrep, grep};
#[cfg(feature = "head")]
pub use head::head;
#[cfg(feature = "nl")]
pub use nl::nl;
#[cfg(feature = "paste")]
pub use paste::paste;
#[cfg(feature = "printf")]
pub use printf::printf;
#[cfg(feature = "rev")]
pub use rev::rev;
#[cfg(feature = "sed")]
pub use sed::sed;
#[cfg(feature = "seq")]
pub use seq::seq;
#[cfg(feature = "sort")]
pub use sort::sort;
#[cfg(feature = "strings")]
pub use strings::strings;
#[cfg(feature = "tac")]
pub use tac::tac;
#[cfg(feature = "tail")]
pub use tail::tail;
#[cfg(feature = "tee")]
pub use tee::tee;
#[cfg(feature = "tr")]
pub use tr::tr;
#[cfg(feature = "unexpand")]
pub use unexpand::unexpand;
#[cfg(feature = "uniq")]
pub use uniq::uniq;
#[cfg(feature = "unix2dos")]
pub use unix2dos::unix2dos;
#[cfg(feature = "wc")]
pub use wc::wc;
#[cfg(feature = "yes")]
pub use yes::yes;
