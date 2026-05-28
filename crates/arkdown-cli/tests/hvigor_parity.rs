//! hvigor/arkdown artifact parity harness for the minimal HAP fixture.
//!
//! This is intentionally ignored: local hvigor availability, signing material,
//! and current arkdown parity are machine-dependent while the implementation is
//! still converging. Run it explicitly to generate a structured JSON report.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde_json::{json, Value};
use walkdir::WalkDir;
use zip::ZipArchive;

const ENV_HVIGORW: &str = "ARKDOWN_PARITY_HVIGORW";
const ENV_ARKDOWN_BIN: &str = "ARKDOWN_PARITY_ARKDOWN_BIN";
const ENV_REPORT: &str = "ARKDOWN_PARITY_REPORT";
const ENV_STRICT: &str = "ARKDOWN_PARITY_STRICT";

#[test]
#[ignore = "requires local hvigorw/DevEco toolchain; run explicitly for artifact parity"]
fn minimal_hap_hvigor_parity_report() {
    let report = run_parity_harness().expect("parity harness failed before report generation");
    let report_path = report["report_path"]
        .as_str()
        .unwrap_or("<unknown report path>");
    let equal = report["equal"].as_bool().unwrap_or(false);
    let diff_count = report["diffs"]
        .as_array()
        .map(|diffs| diffs.len())
        .unwrap_or(0);

    println!(
        "hvigor parity report written to {report_path}; equal={equal}; diff_count={diff_count}"
    );

    if env_truthy(ENV_STRICT) && !equal {
        panic!("strict parity failed; see {report_path}");
    }
}

fn run_parity_harness() -> Result<Value, Box<dyn std::error::Error>> {
    let repo_root = repo_root();
    let fixture = repo_root.join("tests/fixtures/minimal-hap");
    let report_path = report_path(&repo_root);
    fs::create_dir_all(report_path.parent().expect("report path has parent"))?;

    let temp = tempfile::Builder::new()
        .prefix("arkdown-hvigor-parity-")
        .tempdir()?;
    let hvigor_root = temp.path().join("hvigor/minimal-hap");
    let arkdown_root = temp.path().join("arkdown/minimal-hap");
    copy_clean_fixture(&fixture, &hvigor_root)?;
    copy_clean_fixture(&fixture, &arkdown_root)?;

    let hvigorw = hvigorw_program(&hvigor_root);
    let arkdown_bin = arkdown_program();

    let hvigor_command = run_command("hvigor", &hvigorw, &["assembleHap"], &hvigor_root);
    let arkdown_project_arg = arkdown_root.to_string_lossy().into_owned();
    let mut arkdown_args = vec!["build", "--target", "hap", "--mode", "debug"];
    arkdown_args.push("--project");
    arkdown_args.push(arkdown_project_arg.as_str());
    let arkdown_command = run_command("arkdown", &arkdown_bin, &arkdown_args, &arkdown_root);

    let hvigor = collect_build("hvigor", &hvigor_root, hvigor_command)?;
    let arkdown = collect_build("arkdown", &arkdown_root, arkdown_command)?;
    let diffs = compare_builds(&hvigor, &arkdown);

    let equal = diffs.is_empty()
        && hvigor.command.success
        && arkdown.command.success
        && hvigor.command.status == "ran"
        && arkdown.command.status == "ran";

    let mut report = json!({
        "fixture": "tests/fixtures/minimal-hap",
        "report_path": report_path.to_string_lossy(),
        "equal": equal,
        "strict": env_truthy(ENV_STRICT),
        "env": {
            "ARKDOWN_PARITY_HVIGORW": std::env::var(ENV_HVIGORW).ok(),
            "ARKDOWN_PARITY_ARKDOWN_BIN": std::env::var(ENV_ARKDOWN_BIN).ok(),
            "ARKDOWN_PARITY_REPORT": std::env::var(ENV_REPORT).ok(),
        },
        "builds": {
            "hvigor": hvigor.to_json(),
            "arkdown": arkdown.to_json(),
        },
        "diffs": diffs,
    });

    let pretty = serde_json::to_string_pretty(&report)?;
    fs::write(&report_path, pretty)?;
    report["report_path"] = json!(report_path.to_string_lossy());
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn report_path(repo_root: &Path) -> PathBuf {
    std::env::var_os(ENV_REPORT)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("target/parity/minimal-hap-hvigor-diff.json"))
}

fn hvigorw_program(project_root: &Path) -> OsString {
    if let Some(path) = std::env::var_os(ENV_HVIGORW) {
        return path;
    }
    let local = project_root.join("hvigorw");
    if local.exists() {
        return local.into_os_string();
    }
    OsString::from("hvigorw")
}

fn arkdown_program() -> OsString {
    std::env::var_os(ENV_ARKDOWN_BIN)
        .unwrap_or_else(|| OsString::from(env!("CARGO_BIN_EXE_arkdown")))
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn copy_clean_fixture(src: &Path, dst: &Path) -> io::Result<()> {
    for entry in WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_copy_fixture_entry(src, entry.path()))
    {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).expect("entry under fixture");
        if rel.as_os_str().is_empty() {
            continue;
        }

        let target = dst.join(rel);
        let file_type = entry.file_type();
        if file_type.is_dir() {
            fs::create_dir_all(&target)?;
        } else if file_type.is_symlink() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            copy_symlink(entry.path(), &target)?;
        } else if file_type.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
            fs::set_permissions(&target, fs::metadata(entry.path())?.permissions())?;
        }
    }
    Ok(())
}

fn should_copy_fixture_entry(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return true;
    };
    !rel.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        name == "build" || name == ".hvigor" || name == ".idea"
    })
}

#[cfg(unix)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, dst)
}

#[cfg(windows)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    let target = fs::read_link(src)?;
    if src.metadata()?.is_dir() {
        std::os::windows::fs::symlink_dir(target, dst)
    } else {
        std::os::windows::fs::symlink_file(target, dst)
    }
}

#[derive(Clone)]
struct CommandReport {
    tool: String,
    command: Vec<String>,
    cwd: PathBuf,
    status: String,
    success: bool,
    exit_code: Option<i32>,
    duration_ms: u128,
    stdout_tail: String,
    stderr_tail: String,
    error: Option<String>,
}

fn run_command(tool: &str, program: &OsStr, args: &[&str], cwd: &Path) -> CommandReport {
    let start = Instant::now();
    let command = std::iter::once(program.to_string_lossy().into_owned())
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect::<Vec<_>>();

    match Command::new(program).args(args).current_dir(cwd).output() {
        Ok(output) => CommandReport {
            tool: tool.to_string(),
            command,
            cwd: cwd.to_path_buf(),
            status: "ran".to_string(),
            success: output.status.success(),
            exit_code: output.status.code(),
            duration_ms: start.elapsed().as_millis(),
            stdout_tail: tail_string(&output.stdout, 16_384),
            stderr_tail: tail_string(&output.stderr, 16_384),
            error: None,
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => CommandReport {
            tool: tool.to_string(),
            command,
            cwd: cwd.to_path_buf(),
            status: "not_found".to_string(),
            success: false,
            exit_code: None,
            duration_ms: start.elapsed().as_millis(),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            error: Some(err.to_string()),
        },
        Err(err) => CommandReport {
            tool: tool.to_string(),
            command,
            cwd: cwd.to_path_buf(),
            status: "failed_to_start".to_string(),
            success: false,
            exit_code: None,
            duration_ms: start.elapsed().as_millis(),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            error: Some(err.to_string()),
        },
    }
}

fn tail_string(bytes: &[u8], max_len: usize) -> String {
    let start = bytes.len().saturating_sub(max_len);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

impl CommandReport {
    fn to_json(&self) -> Value {
        json!({
            "tool": self.tool,
            "command": self.command,
            "cwd": self.cwd.to_string_lossy(),
            "status": self.status,
            "success": self.success,
            "exit_code": self.exit_code,
            "duration_ms": self.duration_ms,
            "stdout_tail": self.stdout_tail,
            "stderr_tail": self.stderr_tail,
            "error": self.error,
        })
    }
}

#[derive(Clone)]
struct BuildSnapshot {
    tool: String,
    root: PathBuf,
    command: CommandReport,
    hap: Option<HapSnapshot>,
    artifacts: BTreeMap<String, ArtifactSnapshot>,
    loader_out_js: BTreeMap<String, ArtifactSnapshot>,
}

impl BuildSnapshot {
    fn to_json(&self) -> Value {
        let artifacts = self
            .artifacts
            .iter()
            .map(|(name, artifact)| (name.clone(), artifact.to_json()))
            .collect::<serde_json::Map<_, _>>();
        let loader_out_js = self
            .loader_out_js
            .iter()
            .map(|(name, artifact)| (name.clone(), artifact.to_json()))
            .collect::<serde_json::Map<_, _>>();

        json!({
            "tool": self.tool,
            "root": self.root.to_string_lossy(),
            "command": self.command.to_json(),
            "hap": self.hap.as_ref().map(HapSnapshot::to_json),
            "artifacts": artifacts,
            "loader_out_js": loader_out_js,
        })
    }
}

fn collect_build(
    tool: &str,
    root: &Path,
    command: CommandReport,
) -> Result<BuildSnapshot, Box<dyn std::error::Error>> {
    let hap = find_hap(root, tool)
        .map(|path| read_hap_snapshot(&path, root))
        .transpose()?;
    let mut artifacts = BTreeMap::new();

    collect_named_artifact(
        &mut artifacts,
        root,
        hap.as_ref(),
        "module.json",
        Some("module.json"),
        &[
            "intermediates/package",
            "intermediates/res",
            "intermediates/process_profile",
        ],
        false,
    )?;
    collect_named_artifact(
        &mut artifacts,
        root,
        hap.as_ref(),
        "pack.info",
        Some("pack.info"),
        &["outputs", "build/default"],
        false,
    )?;
    collect_named_artifact(
        &mut artifacts,
        root,
        hap.as_ref(),
        "pkgContextInfo.json",
        Some("pkgContextInfo.json"),
        &["pkgContextInfo", "outputs", "build/default"],
        false,
    )?;
    collect_named_artifact(
        &mut artifacts,
        root,
        hap.as_ref(),
        "loader.json",
        None,
        &["intermediates/loader"],
        false,
    )?;
    collect_named_artifact(
        &mut artifacts,
        root,
        hap.as_ref(),
        "modules.abc",
        Some("ets/modules.abc"),
        &["intermediates/ets", "intermediates/loader_out"],
        true,
    )?;
    collect_named_artifact(
        &mut artifacts,
        root,
        hap.as_ref(),
        "sourceMaps.map",
        Some("ets/sourceMaps.map"),
        &["intermediates/source_map", "intermediates/loader_out"],
        false,
    )?;

    Ok(BuildSnapshot {
        tool: tool.to_string(),
        root: root.to_path_buf(),
        command,
        hap,
        artifacts,
        loader_out_js: collect_loader_out_js(root)?,
    })
}

fn find_hap(root: &Path, tool: &str) -> Option<PathBuf> {
    let mut haps = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(OsStr::to_str) == Some("hap"))
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    haps.sort_by(|a, b| {
        let a_score = hap_priority(root, a, tool);
        let b_score = hap_priority(root, b, tool);
        a_score.cmp(&b_score).then_with(|| a.cmp(b))
    });
    haps.into_iter().next()
}

fn hap_priority(root: &Path, path: &Path, tool: &str) -> u8 {
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    if tool == "arkdown" && rel == "build/default/outputs/default/entry.hap" {
        return 0;
    }
    if rel.contains("-signed.") || rel.ends_with("signed.hap") {
        return 1;
    }
    if rel == "build/default/outputs/default/entry.hap" {
        return 2;
    }
    if rel.contains("unsigned") {
        return 3;
    }
    4
}

#[derive(Clone)]
struct HapSnapshot {
    path: PathBuf,
    entries: BTreeMap<String, ZipEntrySnapshot>,
    contents: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone)]
struct ZipEntrySnapshot {
    size: u64,
    compressed_size: u64,
    content_hash: String,
}

impl HapSnapshot {
    fn to_json(&self) -> Value {
        let entries = self
            .entries
            .iter()
            .map(|(name, entry)| {
                (
                    name.clone(),
                    json!({
                        "size": entry.size,
                        "compressed_size": entry.compressed_size,
                        "content_hash": entry.content_hash,
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();

        json!({
            "path": self.path.to_string_lossy(),
            "entry_count": self.entries.len(),
            "entries": entries,
        })
    }
}

fn read_hap_snapshot(path: &Path, root: &Path) -> Result<HapSnapshot, Box<dyn std::error::Error>> {
    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut entries = BTreeMap::new();
    let mut contents = BTreeMap::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            entries.insert(
                file.name().to_string(),
                ZipEntrySnapshot {
                    size: 0,
                    compressed_size: file.compressed_size(),
                    content_hash: stable_hash_hex(&[]),
                },
            );
            continue;
        }

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let normalized = normalize_project_bytes(&bytes, root);
        entries.insert(
            file.name().to_string(),
            ZipEntrySnapshot {
                size: file.size(),
                compressed_size: file.compressed_size(),
                content_hash: stable_hash_hex(&normalized),
            },
        );
        contents.insert(file.name().to_string(), bytes);
    }

    Ok(HapSnapshot {
        path: path.to_path_buf(),
        entries,
        contents,
    })
}

fn collect_named_artifact(
    artifacts: &mut BTreeMap<String, ArtifactSnapshot>,
    root: &Path,
    hap: Option<&HapSnapshot>,
    artifact_name: &str,
    hap_entry: Option<&str>,
    preferred_path_fragments: &[&str],
    binary: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(hap), Some(entry_name)) = (hap, hap_entry) {
        if let Some(bytes) = hap.contents.get(entry_name) {
            artifacts.insert(
                artifact_name.to_string(),
                ArtifactSnapshot::from_bytes(
                    format!("hap:{entry_name}"),
                    bytes.clone(),
                    root,
                    binary,
                ),
            );
            return Ok(());
        }
    }

    if let Some(path) = find_named_file(root, artifact_name, preferred_path_fragments) {
        let bytes = fs::read(&path)?;
        artifacts.insert(
            artifact_name.to_string(),
            ArtifactSnapshot::from_bytes(rel_source(root, &path), bytes, root, binary),
        );
    }
    Ok(())
}

fn find_named_file(root: &Path, file_name: &str, preferred_fragments: &[&str]) -> Option<PathBuf> {
    let mut candidates = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == OsStr::new(file_name))
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        let a_score = preferred_path_score(root, a, preferred_fragments);
        let b_score = preferred_path_score(root, b, preferred_fragments);
        a_score.cmp(&b_score).then_with(|| a.cmp(b))
    });
    candidates.into_iter().next()
}

fn preferred_path_score(root: &Path, path: &Path, fragments: &[&str]) -> usize {
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    fragments
        .iter()
        .position(|fragment| rel.contains(fragment))
        .unwrap_or(fragments.len())
}

fn collect_loader_out_js(
    root: &Path,
) -> Result<BTreeMap<String, ArtifactSnapshot>, Box<dyn std::error::Error>> {
    let mut files = BTreeMap::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(OsStr::to_str) != Some("js")
            || !entry.path().components().any(
                |component| matches!(component, Component::Normal(name) if name == "loader_out"),
            )
        {
            continue;
        }

        let key = loader_out_key(root, entry.path());
        let bytes = fs::read(entry.path())?;
        let mut unique_key = key.clone();
        if files.contains_key(&unique_key) {
            unique_key = rel_source(root, entry.path());
        }
        files.insert(
            unique_key,
            ArtifactSnapshot::from_bytes(rel_source(root, entry.path()), bytes, root, false),
        );
    }
    Ok(files)
}

fn loader_out_key(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut after_loader_out = Vec::new();
    let mut seen_loader_out = false;
    for component in rel.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        if seen_loader_out {
            after_loader_out.push(name.to_string_lossy().into_owned());
        } else if name == "loader_out" {
            seen_loader_out = true;
        }
    }

    if after_loader_out.first().map(String::as_str) == Some("default")
        && after_loader_out.get(1).map(String::as_str) == Some("ets")
    {
        after_loader_out.drain(0..2);
    }

    if after_loader_out.is_empty() {
        rel.to_string_lossy().into_owned()
    } else {
        after_loader_out.join("/")
    }
}

#[derive(Clone)]
struct ArtifactSnapshot {
    source: String,
    kind: ArtifactKind,
    original_len: usize,
}

#[derive(Clone)]
enum ArtifactKind {
    Json(Value),
    Text(String),
    Binary(Vec<u8>),
}

impl ArtifactSnapshot {
    fn from_bytes(source: String, bytes: Vec<u8>, root: &Path, force_binary: bool) -> Self {
        let original_len = bytes.len();
        if force_binary {
            return ArtifactSnapshot {
                source,
                kind: ArtifactKind::Binary(bytes),
                original_len,
            };
        }

        match String::from_utf8(bytes) {
            Ok(text) => {
                let normalized = normalize_project_text(&text, root);
                match serde_json::from_str::<Value>(&normalized) {
                    Ok(value) => ArtifactSnapshot {
                        source,
                        kind: ArtifactKind::Json(value),
                        original_len,
                    },
                    Err(_) => ArtifactSnapshot {
                        source,
                        kind: ArtifactKind::Text(normalized),
                        original_len,
                    },
                }
            }
            Err(err) => ArtifactSnapshot {
                source,
                kind: ArtifactKind::Binary(err.into_bytes()),
                original_len,
            },
        }
    }

    fn to_json(&self) -> Value {
        let normalized = self.normalized_bytes();
        let mut out = json!({
            "source": self.source,
            "kind": self.kind_name(),
            "original_len": self.original_len,
            "normalized_len": normalized.len(),
            "normalized_hash": stable_hash_hex(&normalized),
        });

        if let ArtifactKind::Json(value) = &self.kind {
            out["normalized_json"] = value.clone();
        }

        out
    }

    fn kind_name(&self) -> &'static str {
        match self.kind {
            ArtifactKind::Json(_) => "json",
            ArtifactKind::Text(_) => "text",
            ArtifactKind::Binary(_) => "binary",
        }
    }

    fn normalized_bytes(&self) -> Vec<u8> {
        match &self.kind {
            ArtifactKind::Json(value) => serde_json::to_vec_pretty(value).unwrap_or_default(),
            ArtifactKind::Text(text) => text.as_bytes().to_vec(),
            ArtifactKind::Binary(bytes) => bytes.clone(),
        }
    }
}

fn normalize_project_bytes(bytes: &[u8], root: &Path) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(text) => normalize_project_text(text, root).into_bytes(),
        Err(_) => bytes.to_vec(),
    }
}

fn normalize_project_text(text: &str, root: &Path) -> String {
    let mut normalized = text.to_string();
    for candidate in path_spellings(root) {
        normalized = normalized.replace(&candidate, "<PROJECT>");
    }
    normalized
}

fn path_spellings(path: &Path) -> Vec<String> {
    let mut spellings = vec![path.to_string_lossy().into_owned()];
    if let Ok(canonical) = fs::canonicalize(path) {
        let canonical = canonical.to_string_lossy().into_owned();
        if !spellings.contains(&canonical) {
            spellings.push(canonical);
        }
    }
    spellings
}

fn compare_builds(left: &BuildSnapshot, right: &BuildSnapshot) -> Vec<Value> {
    let mut diffs = Vec::new();

    compare_command(&mut diffs, "hvigor", &left.command);
    compare_command(&mut diffs, "arkdown", &right.command);
    compare_hap_entries(&mut diffs, left.hap.as_ref(), right.hap.as_ref());
    compare_artifact_maps(&mut diffs, "artifacts", &left.artifacts, &right.artifacts);
    compare_artifact_maps(
        &mut diffs,
        "loader_out_js",
        &left.loader_out_js,
        &right.loader_out_js,
    );

    diffs
}

fn compare_command(diffs: &mut Vec<Value>, tool: &str, command: &CommandReport) {
    if command.status != "ran" || !command.success {
        diffs.push(json!({
            "scope": "command",
            "tool": tool,
            "equal": false,
            "status": command.status,
            "exit_code": command.exit_code,
            "error": command.error,
            "stderr_tail": command.stderr_tail,
        }));
    }
}

fn compare_hap_entries(
    diffs: &mut Vec<Value>,
    left: Option<&HapSnapshot>,
    right: Option<&HapSnapshot>,
) {
    match (left, right) {
        (Some(left), Some(right)) => {
            let left_keys = left.entries.keys().cloned().collect::<BTreeSet<_>>();
            let right_keys = right.entries.keys().cloned().collect::<BTreeSet<_>>();
            let left_only = left_keys
                .difference(&right_keys)
                .cloned()
                .collect::<Vec<_>>();
            let right_only = right_keys
                .difference(&left_keys)
                .cloned()
                .collect::<Vec<_>>();
            let mut changed = Vec::new();
            for name in left_keys.intersection(&right_keys) {
                let left_entry = &left.entries[name];
                let right_entry = &right.entries[name];
                if left_entry.size != right_entry.size
                    || left_entry.content_hash != right_entry.content_hash
                {
                    changed.push(json!({
                        "name": name,
                        "left": {
                            "size": left_entry.size,
                            "content_hash": left_entry.content_hash,
                        },
                        "right": {
                            "size": right_entry.size,
                            "content_hash": right_entry.content_hash,
                        },
                    }));
                }
            }

            if !left_only.is_empty() || !right_only.is_empty() || !changed.is_empty() {
                diffs.push(json!({
                    "scope": "hap_entries",
                    "equal": false,
                    "left_only": left_only,
                    "right_only": right_only,
                    "changed": changed,
                }));
            }
        }
        (None, Some(_)) => diffs.push(json!({
            "scope": "hap_entries",
            "equal": false,
            "left_missing": true,
        })),
        (Some(_), None) => diffs.push(json!({
            "scope": "hap_entries",
            "equal": false,
            "right_missing": true,
        })),
        (None, None) => diffs.push(json!({
            "scope": "hap_entries",
            "equal": false,
            "left_missing": true,
            "right_missing": true,
        })),
    }
}

fn compare_artifact_maps(
    diffs: &mut Vec<Value>,
    scope: &str,
    left: &BTreeMap<String, ArtifactSnapshot>,
    right: &BTreeMap<String, ArtifactSnapshot>,
) {
    let left_keys = left.keys().cloned().collect::<BTreeSet<_>>();
    let right_keys = right.keys().cloned().collect::<BTreeSet<_>>();

    for name in left_keys.difference(&right_keys) {
        diffs.push(json!({
            "scope": scope,
            "artifact": name,
            "equal": false,
            "right_missing": true,
            "left": left[name].to_json(),
        }));
    }
    for name in right_keys.difference(&left_keys) {
        diffs.push(json!({
            "scope": scope,
            "artifact": name,
            "equal": false,
            "left_missing": true,
            "right": right[name].to_json(),
        }));
    }
    for name in left_keys.intersection(&right_keys) {
        let artifact_diff = compare_artifact(name, &left[name], &right[name]);
        if !artifact_diff["equal"].as_bool().unwrap_or(false) {
            let mut diff = artifact_diff;
            diff["scope"] = json!(scope);
            diffs.push(diff);
        }
    }
}

fn compare_artifact(name: &str, left: &ArtifactSnapshot, right: &ArtifactSnapshot) -> Value {
    match (&left.kind, &right.kind) {
        (ArtifactKind::Json(left_json), ArtifactKind::Json(right_json)) => {
            let mut changes = Vec::new();
            diff_json("", left_json, right_json, &mut changes);
            json!({
                "artifact": name,
                "equal": changes.is_empty(),
                "kind": "json",
                "left": left.to_json(),
                "right": right.to_json(),
                "json_change_count": changes.len(),
                "json_changes": changes.into_iter().take(200).collect::<Vec<_>>(),
            })
        }
        _ => {
            let left_bytes = left.normalized_bytes();
            let right_bytes = right.normalized_bytes();
            let equal = left_bytes == right_bytes;
            json!({
                "artifact": name,
                "equal": equal,
                "kind": format!("{}:{}", left.kind_name(), right.kind_name()),
                "left": left.to_json(),
                "right": right.to_json(),
                "first_mismatch": if equal {
                    Value::Null
                } else {
                    first_mismatch_json(&left_bytes, &right_bytes)
                },
            })
        }
    }
}

fn diff_json(path: &str, left: &Value, right: &Value, changes: &mut Vec<Value>) {
    if left == right {
        return;
    }

    match (left, right) {
        (Value::Object(left_obj), Value::Object(right_obj)) => {
            let left_keys = left_obj.keys().cloned().collect::<BTreeSet<_>>();
            let right_keys = right_obj.keys().cloned().collect::<BTreeSet<_>>();
            for key in left_keys.difference(&right_keys) {
                changes.push(json!({
                    "path": json_pointer(path, key),
                    "left": left_obj[key],
                    "right_missing": true,
                }));
            }
            for key in right_keys.difference(&left_keys) {
                changes.push(json!({
                    "path": json_pointer(path, key),
                    "left_missing": true,
                    "right": right_obj[key],
                }));
            }
            for key in left_keys.intersection(&right_keys) {
                diff_json(
                    &json_pointer(path, key),
                    &left_obj[key],
                    &right_obj[key],
                    changes,
                );
            }
        }
        (Value::Array(left_arr), Value::Array(right_arr)) => {
            let common = left_arr.len().min(right_arr.len());
            for index in 0..common {
                diff_json(
                    &format!("{path}/{index}"),
                    &left_arr[index],
                    &right_arr[index],
                    changes,
                );
            }
            for (index, value) in left_arr.iter().enumerate().skip(common) {
                changes.push(json!({
                    "path": format!("{path}/{index}"),
                    "left": value,
                    "right_missing": true,
                }));
            }
            for (index, value) in right_arr.iter().enumerate().skip(common) {
                changes.push(json!({
                    "path": format!("{path}/{index}"),
                    "left_missing": true,
                    "right": value,
                }));
            }
        }
        _ => changes.push(json!({
            "path": if path.is_empty() { "/" } else { path },
            "left": left,
            "right": right,
        })),
    }
}

fn json_pointer(parent: &str, key: &str) -> String {
    format!("{}/{}", parent, key.replace('~', "~0").replace('/', "~1"))
}

fn first_mismatch_json(left: &[u8], right: &[u8]) -> Value {
    let offset = left
        .iter()
        .zip(right.iter())
        .position(|(l, r)| l != r)
        .unwrap_or_else(|| left.len().min(right.len()));
    json!({
        "offset": offset,
        "left_byte": left.get(offset).copied(),
        "right_byte": right.get(offset).copied(),
    })
}

fn rel_source(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| format!("<PROJECT>/{}", rel.to_string_lossy()))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn stable_hash_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}
