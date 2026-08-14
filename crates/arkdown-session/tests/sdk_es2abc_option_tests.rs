use arkdown_session::SdkEs2abcOptionSnapshot;

#[test]
fn option_snapshot_normalizes_help_shapes_and_inline_values() {
    let snapshot = SdkEs2abcOptionSnapshot::from_options([
        "  --target-api-version=21",
        "--enable-release-column: emit source columns",
        "--output <file>",
        "--output <duplicate>",
    ]);

    assert!(snapshot.supports("--target-api-version"));
    assert!(snapshot.supports("--target-api-version=24"));
    assert!(snapshot.supports("--enable-release-column"));
    assert!(snapshot.supports("--output"));
    assert!(!snapshot.supports("--enable-callable-name"));
}

#[test]
fn missing_compiler_records_a_typed_probe_failure() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let snapshot = SdkEs2abcOptionSnapshot::inspect(&temporary.path().join("missing-es2abc"));

    assert!(!snapshot.is_available());
    assert!(snapshot.probe_failure().is_some());
}

#[cfg(unix)]
#[test]
fn probe_accepts_options_from_stdout_and_stderr_on_nonzero_help_exit() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().expect("temporary directory");
    let compiler = temporary.path().join("es2abc");
    fs::write(
        &compiler,
        "#!/bin/sh\nprintf '  --target-api-version=<value>\\n'\nprintf '  --enable-release-column\\n' >&2\nexit 1\n",
    )
    .expect("write fake compiler");
    let mut permissions = fs::metadata(&compiler)
        .expect("compiler metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&compiler, permissions).expect("make fake compiler executable");

    let snapshot = SdkEs2abcOptionSnapshot::inspect(&compiler);

    assert!(snapshot.is_available());
    assert!(snapshot.supports("--target-api-version"));
    assert!(snapshot.supports("--enable-release-column"));
    assert!(snapshot.probe_failure().is_none());
}
