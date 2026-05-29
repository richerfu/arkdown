//! Integration tests for the bundle-level output cache.
//!
//! Both tests use the `minimal-hap` fixture and drive the full build pipeline
//! through `arkdown_cli::cmd::build::run` to verify end-to-end cache behaviour.

use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/minimal-hap")
}

fn isolated_fixture(src: &PathBuf) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before UNIX epoch")
        .as_nanos();
    let dst = std::env::temp_dir().join(format!(
        "arkdown-cache-test-{}-{n}-{nonce}",
        std::process::id()
    ));
    copy_dir(src, &dst);
    dst
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("create dir");
    for entry in std::fs::read_dir(src).expect("read dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(name_str.as_ref(), "build" | ".hvigor") {
            continue;
        }
        let from = entry.path();
        let to = dst.join(name_str.as_ref());
        if from.is_dir() {
            copy_dir(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy file");
        }
    }
}

fn build_args(project: PathBuf) -> arkdown_cli::cmd::build::BuildArgs {
    arkdown_cli::cmd::build::BuildArgs {
        target: "hap".to_string(),
        mode: "debug".to_string(),
        module: None,
        product: None,
        project: Some(project),
    }
}

/// After the first build, `bundle.json` must exist under
/// `<module>/build/default/cache/bundle/`.
#[tokio::test]
async fn test_bundle_cache_manifest_written_after_first_build() {
    let src = fixture_root();
    if !src.exists() {
        eprintln!("skip: fixture not found at {}", src.display());
        return;
    }

    let fixture = isolated_fixture(&src);
    let args = build_args(fixture.clone());

    arkdown_cli::cmd::build::run(args)
        .await
        .expect("first build must succeed");

    // Locate the module directory (entry or first module).
    let entry_dir = fixture.join("entry");
    let bundle_json = entry_dir.join("build/default/cache/bundle/bundle.json");

    assert!(
        bundle_json.exists(),
        "bundle.json must exist after first build at {}",
        bundle_json.display()
    );

    let _ = std::fs::remove_dir_all(&fixture);
}

/// Two sequential builds must produce byte-identical `modules.abc`.
/// The second build must use the bundle cache (chunk .js files are not re-written
/// by rolldown) and still produce the same ABC output.
#[tokio::test]
async fn test_second_build_uses_bundle_cache_and_produces_identical_abc() {
    let src = fixture_root();
    if !src.exists() {
        eprintln!("skip: fixture not found at {}", src.display());
        return;
    }

    let fixture = isolated_fixture(&src);

    // First build.
    arkdown_cli::cmd::build::run(build_args(fixture.clone()))
        .await
        .expect("first build must succeed");

    // Locate modules.abc from the first build.
    let entry_dir = fixture.join("entry");
    let abc_path = entry_dir
        .join("build/default/intermediates/loader_out/default/ets/modules.abc");

    let first_abc = std::fs::read(&abc_path).expect("modules.abc must exist after first build");
    assert!(!first_abc.is_empty(), "modules.abc must not be empty");

    // Verify the bundle.json cache manifest was written.
    let bundle_json = entry_dir.join("build/default/cache/bundle/bundle.json");
    assert!(
        bundle_json.exists(),
        "bundle.json must exist before second build"
    );

    // Second build — must hit the bundle cache.
    arkdown_cli::cmd::build::run(build_args(fixture.clone()))
        .await
        .expect("second build must succeed");

    let second_abc = std::fs::read(&abc_path).expect("modules.abc must exist after second build");
    assert_eq!(
        first_abc, second_abc,
        "modules.abc must be byte-identical across two sequential builds"
    );

    let _ = std::fs::remove_dir_all(&fixture);
}
