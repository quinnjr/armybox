//! Shared utility traits for the ABP package manager.

extern crate alloc;
use alloc::string::String;

pub trait ToLowercase {
    fn to_lowercase(&self) -> String;
}

impl ToLowercase for str {
    fn to_lowercase(&self) -> String {
        self.chars()
            .map(|c: char| {
                if c.is_ascii_uppercase() {
                    (c as u8 + 32) as char
                } else {
                    c
                }
            })
            .collect()
    }
}
