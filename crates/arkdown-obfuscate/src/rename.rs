//! Short-name allocator for identifier renaming.
//!
//! Produces names in lexicographic sequence: `a`, `b`, …, `z`, `aa`, `ab`,
//! …, `az`, `ba`, …  Skips any name that appears in the reserved set (the
//! `-keep-global-name` / `-keep-property-name` lists) and any JavaScript
//! reserved keyword.

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
            let name = encode_name(self.counter);
            self.counter += 1;
            if !self.reserved.contains(&name) && !is_js_reserved(&name) {
                return name;
            }
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

/// Return `true` if `name` is a JavaScript / TypeScript reserved word that
/// must not be used as an identifier.
fn is_js_reserved(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "in"
            | "instanceof"
            | "new"
            | "return"
            | "switch"
            | "this"
            | "throw"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "class"
            | "const"
            | "enum"
            | "export"
            | "extends"
            | "import"
            | "super"
            | "implements"
            | "interface"
            | "let"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
            | "yield"
            | "null"
            | "true"
            | "false"
    )
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
