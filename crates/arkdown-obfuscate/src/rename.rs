//! Short-name allocator for identifier renaming.
//!
//! Produces names in lexicographic sequence: `a`, `b`, …, `z`, `aa`, `ab`,
//! …, `az`, `ba`, …  Skips any name that appears in the reserved set (the
//! `-keep-global-name` / `-keep-property-name` lists) and any JavaScript
//! reserved keyword.

use oxc_syntax::{identifier::is_identifier_name, keyword::is_reserved_keyword};
use rustc_hash::FxHashSet;

/// Generates short identifier names in a deterministic sequence.
pub struct NameAllocator {
    counter: usize,
    reserved: FxHashSet<String>,
}

impl NameAllocator {
    /// Create a new allocator.  `reserved` is the set of names that must never
    /// be emitted (keep-lists + JS reserved words are both filtered).
    pub fn new(reserved: FxHashSet<String>) -> Self {
        NameAllocator {
            counter: 0,
            reserved,
        }
    }

    /// Return the next available short name and advance the counter.
    pub fn next(&mut self) -> String {
        loop {
            let name = Self::encode_name(self.counter);
            self.counter += 1;
            if !self.reserved.contains(&name) && JsBindingName::new(&name).is_available() {
                return name;
            }
        }
    }

    /// Encode `n` as a base-26 lowercase string (a, b, …, z, aa, ab, …).
    fn encode_name(mut n: usize) -> String {
        const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
        let base = CHARS.len();
        let mut result = Vec::new();
        result.push(CHARS[n % base]);
        n /= base;
        while n > 0 {
            n -= 1;
            result.push(CHARS[n % base]);
            n /= base;
        }
        result.reverse();
        // SAFETY: only ASCII lowercase letters in result
        String::from_utf8(result).expect("encode_name produced non-UTF8")
    }
}

struct JsBindingName<'a> {
    name: &'a str,
}

impl<'a> JsBindingName<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn is_available(&self) -> bool {
        is_identifier_name(self.name) && !is_reserved_keyword(self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence() {
        let mut alloc = NameAllocator::new(FxHashSet::default());
        let names: Vec<_> = (0..28).map(|_| alloc.next()).collect();
        // a..z then aa, ab
        assert_eq!(names[0], "a");
        assert_eq!(names[25], "z");
        assert_eq!(names[26], "aa");
        assert_eq!(names[27], "ab");
    }

    #[test]
    fn test_skips_reserved() {
        let mut reserved = FxHashSet::default();
        // "a" is at position 0; skip it
        reserved.insert("a".to_string());
        let mut alloc = NameAllocator::new(reserved);
        assert_eq!(alloc.next(), "b");
    }
}
