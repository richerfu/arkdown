use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::{ArkdownConfig, SourceReadError};
use arkdown_session::fingerprint::{
    FingerprintError, InputFingerprintRequest, InputFingerprintSet,
};
use arkdown_session::{BuildSession, BuildSessionBuilder};

struct SourceProject {
    _temp: tempfile::TempDir,
    root: PathBuf,
    config: Arc<ArkdownConfig>,
}

impl SourceProject {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        // Match ArkdownConfigLoader's normalized input boundary: std canonicalize
        // retains Windows verbatim prefixes while snapshot identities use AbsPath.
        let root = arkdown_path::AbsPath::canonicalize(temp.path())
            .unwrap()
            .into_path_buf();
        let loader = root.join("sdk/ets/build-tools/ets-loader");
        std::fs::create_dir_all(&loader).unwrap();
        let config = serde_json::from_value(serde_json::json!({
            "project_root": root,
            "build_mode": "Debug", "output_target": "Hap",
            "product": {
                "name": "default", "bundle_name": "test.source.inputs",
                "compatible_sdk_version": "12", "runtime_os": "OpenHarmony",
                "build_option": {"strict_mode": {"case_sensitive_check": false, "use_normalized_ohm_url": true}}
            },
            "modules": [
                {"name": "entry", "src_path": root.join("entry"), "module_type": "Entry"},
                {"name": "other", "src_path": root.join("other"), "module_type": "Entry"}
            ],
            "sdk_config": {
                "ets_loader_path": loader, "es2abc_path": root.join("sdk/es2abc"),
                "restool_path": root.join("sdk/restool"), "compile_api_version": 12,
                "sdk_component_api_version": 12, "has_rollup_plugin": true,
                "compile_sdk_version": "6.0.0.47"
            }
        })).unwrap();
        let fixture = Self {
            _temp: temp,
            root,
            config: Arc::new(config),
        };
        fixture.write(
            "oh-package.json5",
            r#"{name:"source-project",version:"1.0.0"}"#,
        );
        fixture.write(
            "build-profile.json5",
            r#"{app:{products:[{name:"default"}]},modules:[]}"#,
        );
        for module in ["entry", "other"] {
            fixture.write(
                &format!("{module}/build-profile.json5"),
                r#"{apiType:"stageMode",targets:[{name:"default"}]}"#,
            );
            fixture.write(
                &format!("{module}/src/main/module.json5"),
                &format!(r#"{{module:{{name:"{module}",type:"entry"}}}}"#),
            );
        }
        fixture.write(
            "entry/oh-package.json5",
            r#"{name:"entry",version:"1.0.0",dependencies:{library:"1.0.0"}}"#,
        );
        fixture.write(
            "other/oh-package.json5",
            r#"{name:"other",version:"1.0.0"}"#,
        );
        fixture.write(
            "entry/src/main/ets/index.ets",
            "export { value } from 'library';",
        );
        fixture.write(
            "other/src/main/ets/index.ets",
            "export const independent = 1;",
        );
        fixture.write("oh_modules/library/oh-package.json5", r#"{name:"library",version:"1.0.0",main:"index.ets",dependencies:{"libnative.so":"1.0.0"}}"#);
        fixture.write(
            "oh_modules/library/src/main/module.json5",
            r#"{module:{name:"library",type:"har"}}"#,
        );
        fixture.write(
            "oh_modules/library/index.ets",
            "export { value } from './lib/value';",
        );
        fixture.write(
            "oh_modules/library/lib/value.ets",
            "import data from '../data/value.json'; export const value = data.value;",
        );
        fixture.write("oh_modules/library/data/value.json", r#"{"value":1}"#);
        fixture.write(
            "oh_modules/libnative.so/oh-package.json5",
            r#"{name:"libnative.so",version:"1.0.0",types:"Index.d.ts"}"#,
        );
        fixture.write(
            "oh_modules/libnative.so/Index.d.ts",
            "/// <reference path='./detail.d.ts' />\nexport declare const native: NativeValue;",
        );
        fixture.write(
            "oh_modules/libnative.so/detail.d.ts",
            "interface NativeValue { value: number }",
        );
        fixture
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn session(&self, threads: usize) -> BuildSession {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                BuildSessionBuilder::new(Arc::clone(&self.config))
                    .build()
                    .unwrap()
            })
    }

    fn captured(&self, session: &BuildSession, relative: &str) -> bool {
        session
            .source_store
            .source_by_path(&self.root.join(relative))
            .is_some()
    }
}

// ets_checker.ts::resolveModuleNames + CMake file-api v1 Reply Files:
// replacement of native working files must not invalidate source snapshots.
#[test]
fn cmake_reply_replacement_is_outside_source_snapshots_and_fingerprints() {
    let fixture = SourceProject::new();
    fixture.write("oh_modules/library/oh-package.json5", r#"{name:"library",version:"1.0.0",main:"index.ets",metadata:{sourceRoots:["."]},dependencies:{"libnative.so":"1.0.0"}}"#);
    let old = fixture.write("oh_modules/library/.cxx/default/default/debug/arm64-v8a/.cmake/api/v1/reply/cache-v2-old.json", "{}");
    // Invalid UTF-8 proves the unrelated package file is not even read.
    let orphan = fixture.write("oh_modules/library/orphan.json", "");
    std::fs::write(&orphan, [0xff]).unwrap();
    let initial = fixture.session(1);
    assert!(initial.source_store.source_by_path(&old).is_none());
    assert!(initial.source_store.source_by_path(&orphan).is_none());
    for path in [
        "oh_modules/library/index.ets",
        "oh_modules/library/lib/value.ets",
        "oh_modules/library/data/value.json",
        "oh_modules/libnative.so/Index.d.ts",
        "oh_modules/libnative.so/detail.d.ts",
    ] {
        assert!(
            fixture.captured(&initial, path),
            "missing declared/referenced input {path}"
        );
    }
    let evicted = initial.source_store.evict_unleased_texts();
    std::fs::remove_file(&old).unwrap();
    fixture.write("oh_modules/library/.cxx/default/default/debug/arm64-v8a/.cmake/api/v1/reply/cache-v2-new.json", "{\"changed\":true}");
    assert_eq!(
        initial.source_store.restore_evicted_texts().unwrap(),
        evicted
    );
    let rebuilt = fixture.session(4);
    assert_eq!(initial.fingerprints.sources, rebuilt.fingerprints.sources);
    assert_eq!(
        initial.fingerprints.module("entry").unwrap().sources,
        rebuilt.fingerprints.module("entry").unwrap().sources
    );

    fixture.write("oh_modules/library/data/value.json", r#"{"value":2}"#);
    let changed = fixture.session(4);
    assert_ne!(
        initial.fingerprints.module("entry").unwrap().sources,
        changed.fingerprints.module("entry").unwrap().sources
    );
    assert_eq!(
        initial.fingerprints.module("other").unwrap().sources,
        changed.fingerprints.module("other").unwrap().sources
    );
}

#[test]
fn declared_directory_entries_follow_imports_without_walking_the_package_root() {
    let fixture = SourceProject::new();
    fixture.write(
        "oh_modules/library/oh-package.json5",
        r#"{name:"library",version:"1.0.0",main:"published"}"#,
    );
    fixture.write(
        "oh_modules/library/published/index.ets",
        r#"
        export { value } from '../lib/value';
        export const deferred = () => import('../async/value');
        import lazy { value as lazyValue } from '../lazy/value';
        export type Details = import('../types/extra').Details;
        import legacy = require('../legacy/value.cjs');
        const required = require('../legacy/other.js');
        import config from '../.hidden/config.json';
        export { shared } from '../../shared/value';
    "#,
    );
    for path in [
        "async/value.ets",
        "lazy/value.ets",
        "types/extra.d.ts",
        "legacy/value.cjs",
        "legacy/other.js",
        "legacy/other.d.ets",
        ".hidden/config.json",
    ] {
        fixture.write(
            &format!("oh_modules/library/{path}"),
            if path.ends_with("json") {
                "{}"
            } else if path.ends_with(".cjs") {
                "module.exports = { value: 1 };"
            } else if path.ends_with(".d.ts") {
                "export interface Details { value: number }"
            } else if path.ends_with(".d.ets") {
                "export declare const value: number;"
            } else {
                "export const value = 1;"
            },
        );
    }
    fixture.write("oh_modules/shared/value.ts", "export const shared = 1;");
    // The old entry is not referenced by the directory entry.
    let unused = fixture.write("oh_modules/library/not-an-input.ets", "");
    std::fs::write(&unused, [0xff]).unwrap();
    let initial = fixture.session(1);
    for path in [
        "published/index.ets",
        "async/value.ets",
        "lazy/value.ets",
        "types/extra.d.ts",
        "legacy/value.cjs",
        "legacy/other.js",
        "legacy/other.d.ets",
        ".hidden/config.json",
    ] {
        assert!(
            fixture.captured(&initial, &format!("oh_modules/library/{path}")),
            "missing referenced source {path}"
        );
    }
    assert!(fixture.captured(&initial, "oh_modules/shared/value.ts"));
    assert!(initial.source_store.source_by_path(&unused).is_none());
    fixture.write("oh_modules/shared/value.ts", "export const shared = 2;");
    let changed = fixture.session(1);
    assert_ne!(
        initial.fingerprints.module("entry").unwrap().sources,
        changed.fingerprints.module("entry").unwrap().sources
    );
    assert_eq!(
        initial.fingerprints.module("other").unwrap().sources,
        changed.fingerprints.module("other").unwrap().sources
    );
}

#[test]
fn captured_source_deletion_and_change_still_fail_verified_reload() {
    let fixture = SourceProject::new();
    let session = fixture.session(1);
    session.source_store.evict_unleased_texts();
    let source = fixture.root.join("oh_modules/library/lib/value.ets");
    std::fs::remove_file(&source).unwrap();
    let error = session.source_store.restore_evicted_texts().unwrap_err();
    assert!(matches!(error, SourceReadError::ReloadIo { path, .. } if path == source));
    std::fs::write(&source, "export const changed = true;").unwrap();
    let error = session.source_store.restore_evicted_texts().unwrap_err();
    assert!(matches!(error, SourceReadError::SnapshotMismatch { path, .. } if path == source));
}

#[test]
fn source_ids_order_and_fingerprints_match_across_inventory_thread_counts() {
    let fixture = SourceProject::new();
    let serial = fixture.session(1);
    let parallel = fixture.session(4);
    let inventory = |session: &BuildSession| {
        session
            .source_store
            .source_files()
            .into_iter()
            .map(|file| (file.id, file.path.clone(), file.fingerprint.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(inventory(&serial), inventory(&parallel));
    assert_eq!(serial.fingerprints.sources, parallel.fingerprints.sources);
    for name in ["entry", "other"] {
        assert_eq!(
            serial.fingerprints.module(name).unwrap().sources,
            parallel.fingerprints.module(name).unwrap().sources
        );
    }
    let paths = serial.source_store.source_files();
    assert!(paths.windows(2).all(|pair| pair[0].path < pair[1].path));
    assert!(paths
        .iter()
        .enumerate()
        .all(|(index, file)| file.id.index() == index));
}

#[test]
fn fingerprinting_reads_captured_source_bytes_and_propagates_verified_reload_failure() {
    let fixture = SourceProject::new();
    let session = fixture.session(1);
    let source = fixture.root.join("oh_modules/library/data/value.json");
    std::fs::remove_file(&source).unwrap();
    let fingerprint = || {
        InputFingerprintSet::build(InputFingerprintRequest {
            config: &fixture.config,
            project: &session.project,
            profiles: &session.profiles,
            sdk: &session.sdk,
            custom_config: &session.custom_config,
            arkts_language_version: session.arkts_language_version,
            source_store: &session.source_store,
        })
    };
    // An attempted second source read would fail: the startup bytes must be
    // shared with fingerprinting, even after the source disappears from disk.
    assert_eq!(fingerprint().unwrap().sources, session.fingerprints.sources);
    session.source_store.evict_unleased_texts();
    let error = fingerprint().unwrap_err();
    assert!(matches!(&error, FingerprintError::Io { path, .. } if path == &source));
    assert!(error.to_string().contains("ARKDOWN_SOURCE_RELOAD_IO"));
}

#[test]
fn resources_and_declared_exports_keep_their_own_fingerprint_scope() {
    let fixture = SourceProject::new();
    fixture.write(
        "entry/src/main/resources/base/element/string.json",
        r#"{"string":[]}"#,
    );
    fixture.write(
        "oh_modules/library/oh-package.json5",
        r#"{name:"library",version:"1.0.0",main:"index.ets","oh-exports":["api"]}"#,
    );
    fixture.write("oh_modules/library/api/public.ets", "export const api = 1;");
    let initial = fixture.session(1);
    assert!(fixture.captured(&initial, "oh_modules/library/api/public.ets"));
    fixture.write(
        "entry/src/main/resources/base/element/string.json",
        r#"{"string":[{"name":"app_name","value":"changed"}]}"#,
    );
    let resources_changed = fixture.session(1);
    assert_eq!(
        initial.fingerprints.sources,
        resources_changed.fingerprints.sources
    );
    assert_ne!(
        initial.fingerprints.resources,
        resources_changed.fingerprints.resources
    );
    fixture.write("oh_modules/library/api/public.ets", "export const api = 2;");
    let exports_changed = fixture.session(1);
    assert_ne!(
        resources_changed.fingerprints.sources,
        exports_changed.fingerprints.sources
    );
    assert_eq!(
        resources_changed
            .fingerprints
            .module("other")
            .unwrap()
            .sources,
        exports_changed
            .fingerprints
            .module("other")
            .unwrap()
            .sources
    );
}

#[test]
fn referenced_case_aliases_keep_one_physical_source_identity() {
    let fixture = SourceProject::new();
    let original = fixture.root.join("oh_modules/library/lib/value.ets");
    let physical = fixture.root.join("oh_modules/library/lib/Value.ets");
    std::fs::rename(&original, &physical).unwrap();
    if !original.is_file() {
        return;
    }
    fixture.write("oh_modules/library/index.ets", "export { value as first } from './lib/Value'; export { value as second } from './LIB/value';");
    let session = fixture.session(4);
    let source = session.source_store.source_by_path(&original).unwrap();
    assert_eq!(source.path, physical);
    let matches = session
        .source_store
        .source_files()
        .into_iter()
        .filter(|file| {
            file.path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("Value.ets"))
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, source.id);
}

#[cfg(unix)]
#[test]
fn materialized_ohpm_aliases_reuse_the_referenced_physical_source() {
    let fixture = SourceProject::new();
    let alias = fixture.root.join("oh_modules/library");
    let physical = fixture
        .root
        .join("oh_modules/.ohpm/library@1.0.0/oh_modules/library");
    std::fs::create_dir_all(physical.parent().unwrap()).unwrap();
    std::fs::rename(&alias, &physical).unwrap();
    std::os::unix::fs::symlink(&physical, &alias).unwrap();
    let session = fixture.session(4);
    for relative in ["index.ets", "lib/value.ets", "data/value.json"] {
        let from_alias = session
            .source_store
            .source_by_path(&alias.join(relative))
            .unwrap();
        let from_physical = session
            .source_store
            .source_by_path(&physical.join(relative))
            .unwrap();
        assert_eq!(from_alias.id, from_physical.id);
        assert_eq!(from_alias.path, physical.join(relative));
    }
}

#[test]
#[ignore = "requires ARKDOWN_SOURCE_INPUT_PROJECT and an installed SDK"]
fn real_project_inventory_excludes_unreferenced_cmake_replies() {
    let project =
        PathBuf::from(std::env::var_os("ARKDOWN_SOURCE_INPUT_PROJECT").expect("project path"));
    let module = std::env::var("ARKDOWN_SOURCE_INPUT_MODULE").ok();
    let config = arkdown_config::load_config(
        &project,
        arkdown_core::BuildMode::Debug,
        arkdown_core::OutputTarget::Hap,
        Some("default"),
        module.as_deref(),
    )
    .unwrap();
    let session = BuildSessionBuilder::new(Arc::new(config)).build().unwrap();
    for file in session.source_store.source_files() {
        let cmake_reply = file
            .path
            .components()
            .any(|component| component.as_os_str() == ".cxx")
            && file
                .path
                .components()
                .any(|component| component.as_os_str() == ".cmake")
            && file
                .path
                .extension()
                .is_some_and(|extension| extension == "json");
        assert!(
            !cmake_reply,
            "native working file captured as source: {}",
            file.path.display()
        );
    }
    let evicted = session.source_store.evict_unleased_texts();
    assert_eq!(
        session.source_store.restore_evicted_texts().unwrap(),
        evicted
    );
    eprintln!(
        "verified {} source snapshots and their reloads",
        session.source_store.len()
    );
}

#[cfg(unix)]
#[test]
fn local_module_references_use_the_owning_modules_source_roots() {
    let fixture = SourceProject::new();
    fixture.write(
        "entry/oh-package.json5",
        r#"{name:"entry",version:"1.0.0",dependencies:{other:"1.0.0"}}"#,
    );
    fixture.write(
        "other/oh-package.json5",
        r#"{name:"other",version:"1.0.0",main:"src/main/ets/index.ets"}"#,
    );
    fixture.write(
        "entry/src/main/ets/index.ets",
        "export { value } from 'other';",
    );
    fixture.write("entry/src/main/ets/choice.ets", "export default 1;");
    fixture.write(
        "other/src/main/ets/index.ets",
        "export { default as value } from 'choice';",
    );
    fixture.write("other/src/main/ets/choice.ets", "export default 2;");
    std::os::unix::fs::symlink(
        fixture.root.join("other"),
        fixture.root.join("oh_modules/other"),
    )
    .unwrap();
    let initial = fixture.session(1);
    fixture.write("entry/src/main/ets/choice.ets", "export default 3;");
    let changed = fixture.session(4);
    assert_eq!(
        initial.fingerprints.module("other").unwrap().sources,
        changed.fingerprints.module("other").unwrap().sources
    );
    assert_ne!(
        initial.fingerprints.module("entry").unwrap().sources,
        changed.fingerprints.module("entry").unwrap().sources
    );
}

#[cfg(unix)]
#[test]
fn symlinked_module_source_sets_capture_their_references_and_fingerprints() {
    let fixture = SourceProject::new();
    let alias = fixture.root.join("entry/src/main/ets");
    let physical = fixture.root.join("source-tree");
    std::fs::rename(&alias, &physical).unwrap();
    std::os::unix::fs::symlink(&physical, &alias).unwrap();
    fixture.write(
        "source-tree/index.ets",
        "export { shared } from '../shared/value';",
    );
    fixture.write("shared/value.ets", "export const shared = 1;");
    let initial = fixture.session(1);
    assert_eq!(
        initial
            .source_store
            .source_by_path(&alias.join("index.ets"))
            .unwrap()
            .path,
        physical.join("index.ets")
    );
    assert!(fixture.captured(&initial, "shared/value.ets"));
    fixture.write("shared/value.ets", "export const shared = 2;");
    let changed = fixture.session(4);
    assert_ne!(
        initial.fingerprints.module("entry").unwrap().sources,
        changed.fingerprints.module("entry").unwrap().sources
    );
    assert_eq!(
        initial.fingerprints.module("other").unwrap().sources,
        changed.fingerprints.module("other").unwrap().sources
    );
}
