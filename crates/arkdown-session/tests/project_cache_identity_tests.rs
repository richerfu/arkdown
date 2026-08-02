use arkdown_session::BuildScriptProjectCacheIdentity;
use sha2::{Digest, Sha256};

#[test]
fn latest_hvigor_project_cache_identity_uses_normalized_root_and_32_hex_chars() {
    let current = std::env::current_dir().expect("current directory should resolve");
    let root = current.join("fixtures").join("..").join("fixtures");
    let identity = BuildScriptProjectCacheIdentity::new(&root).expect("identity should resolve");
    let normalized = current.join("fixtures");
    let expected = format!(
        "{:x}",
        Sha256::digest(normalized.to_string_lossy().as_bytes())
    );

    assert_eq!(identity.hash(), expected[..32]);
    assert_eq!(identity.hash().len(), 32);
    assert_eq!(
        identity.workspace(&current.join(".hvigor")),
        current
            .join(".hvigor")
            .join("project_caches")
            .join(&expected[..32])
            .join("workspace")
    );
}
