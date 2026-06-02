/// FNV-1a 64-bit hash of a string slice.
pub fn fnv1a_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Convert a path to a forward-slash string (Windows compat).
pub fn to_unix_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Canonicalize a path, falling back to the original if canonicalization fails.
pub fn canonicalize_or_owned(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
