//! Shared archive-path and package-tool JSON helpers for HAP/HSP assembly.

use std::path::{Path, PathBuf};

use arkdown_path::{AbsPath, ArtifactPath, ArtifactPathSerializer};

use crate::error::PackError;
use crate::zip_writer;

use super::{PackInput, Packager};

// Private helpers for HAP/HSP assembly
impl Packager {
    pub(super) fn add_loader_out_ets_assets<W: std::io::Write + std::io::Seek>(
        writer: &mut zip_writer::RawZipWriter<W>,
        ets_dir: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Result<(), PackError> {
        let output_canonical = AbsPath::canonicalize(output_path)
            .ok()
            .map(AbsPath::into_path_buf);
        for entry in walkdir::WalkDir::new(ets_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            let rel = path.strip_prefix(ets_dir).unwrap_or(path);
            let rel_text = ArtifactPathSerializer::rel_to_slash(rel);
            if LoaderOutEtsAsset::new(path, rel, output_path, output_canonical.as_deref())
                .is_generated_artifact()
            {
                continue;
            }
            let dest = ArchivePath::ets(&rel_text);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "json" {
                add_json_resource_stored(writer, &dest, path)?;
            } else {
                zip_writer::add_file_stored(writer, &dest, path)?;
            }
        }
        Ok(())
    }

    pub(super) fn add_aot_outputs<W: std::io::Write + std::io::Seek>(
        writer: &mut zip_writer::RawZipWriter<W>,
        input: &PackInput,
    ) -> Result<(), PackError> {
        let Some(an_dir) = PackAuxiliaryArtifacts::new(input).an_dir() else {
            return Ok(());
        };
        if !an_dir.is_dir() {
            return Ok(());
        }
        for entry in walkdir::WalkDir::new(&an_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let relative = entry.path().strip_prefix(&an_dir).unwrap_or(entry.path());
            zip_writer::add_file_stored(writer, &ArchivePath::aot(relative), entry.path())?;
        }
        Ok(())
    }

    pub(super) fn add_profile_guided_ap<W: std::io::Write + std::io::Seek>(
        writer: &mut zip_writer::RawZipWriter<W>,
        input: &PackInput,
    ) -> Result<(), PackError> {
        let Some(ap_dir) = PackAuxiliaryArtifacts::new(input).ap_dir() else {
            return Ok(());
        };
        let ap_file = ArchivePath::profile_guided_ap_file(&input.module_name);
        let ap_path = ap_dir.join(&ap_file);
        if ap_path.exists() {
            zip_writer::add_file_stored(
                writer,
                &ArchivePath::profile_guided_ap(&input.module_name),
                &ap_path,
            )?;
        }
        Ok(())
    }
}

/// Resolves optional pack inputs from the typed CompileArkTS output layout.
/// This deliberately starts from `abc_path`; it never reverse-engineers a
/// module root from the final output filename or embeds a product/build path.
struct PackAuxiliaryArtifacts<'a> {
    input: &'a PackInput,
}

impl<'a> PackAuxiliaryArtifacts<'a> {
    fn new(input: &'a PackInput) -> Self {
        Self { input }
    }

    fn an_dir(&self) -> Option<PathBuf> {
        self.loader_target_dir().map(|path| path.join("an"))
    }

    fn ap_dir(&self) -> Option<PathBuf> {
        self.intermediates_dir().map(|path| path.join("ap"))
    }

    fn loader_target_dir(&self) -> Option<&Path> {
        let ets_dir = self.input.abc_path.parent()?;
        (ets_dir.file_name() == Some(std::ffi::OsStr::new("ets")))
            .then(|| ets_dir.parent())
            .flatten()
    }

    fn intermediates_dir(&self) -> Option<&Path> {
        let loader_target = self.loader_target_dir()?;
        let loader_out = loader_target.parent()?;
        (loader_out.file_name() == Some(std::ffi::OsStr::new("loader_out")))
            .then(|| loader_out.parent())
            .flatten()
    }
}

struct LoaderOutEtsAsset<'a> {
    path: &'a Path,
    rel: &'a Path,
    output_path: &'a Path,
    output_canonical: Option<&'a Path>,
}

impl<'a> LoaderOutEtsAsset<'a> {
    fn new(
        path: &'a Path,
        rel: &'a Path,
        output_path: &'a Path,
        output_canonical: Option<&'a Path>,
    ) -> Self {
        Self {
            path,
            rel,
            output_path,
            output_canonical,
        }
    }

    fn is_generated_artifact(&self) -> bool {
        self.rel == Path::new("modules.abc")
            || self.rel == Path::new("sourceMaps.map")
            || self.rel == Path::new("filesInfo.txt")
            || self.rel == Path::new("npmEntries.txt")
            || self.rel == Path::new("npmEntries.protoBin")
            || self.is_cache_artifact()
            || self.is_current_output()
            || arkdown_path::SourceFileName::new(self.path).has_extension("js")
    }

    fn is_cache_artifact(&self) -> bool {
        matches!(
            self.rel.components().next(),
            Some(std::path::Component::Normal(name)) if name == "cache"
        )
    }

    fn is_current_output(&self) -> bool {
        if self.path == self.output_path {
            return true;
        }
        self.output_canonical
            .and_then(|output| {
                AbsPath::canonicalize(self.path)
                    .ok()
                    .map(|path| path.as_path() == output)
            })
            .unwrap_or(false)
    }
}

pub(super) struct ArchivePath;

impl ArchivePath {
    pub(super) fn resources(rel: &Path) -> String {
        Self::prefixed_path("resources", rel)
    }

    pub(super) fn native_lib(path: &Path) -> String {
        const NATIVE_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];

        let components = path
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(name) => name.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(index) = components
            .iter()
            .position(|component| NATIVE_ABI_DIRS.contains(component))
        {
            let mut rel = PathBuf::from("libs");
            for component in &components[index..] {
                rel.push(component);
            }
            return ArtifactPathSerializer::rel_to_slash(&rel);
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.so");
        ArtifactPathSerializer::rel_to_slash(
            &PathBuf::from("libs").join("arm64-v8a").join(filename),
        )
    }

    pub(super) fn integrated_hsp(dir_name: &str, file_name: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(&PathBuf::from("libs").join(dir_name).join(file_name))
    }

    pub(super) fn har_resources(rel: &Path) -> String {
        Self::prefixed_path("src/main/resources", rel)
    }

    pub(super) fn ets(rel_text: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(&PathBuf::from("ets").join(rel_text))
    }

    pub(super) fn aot(rel: &Path) -> String {
        Self::prefixed_path("an", rel)
    }

    pub(super) fn profile_guided_ap(module_name: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(
            &PathBuf::from("ap").join(Self::profile_guided_ap_file(module_name)),
        )
    }

    pub(super) fn profile_guided_ap_file(module_name: &str) -> String {
        ProfileGuidedApFileName::new(module_name).file_name()
    }

    pub(super) fn prefixed_path(prefix: &str, rel: &Path) -> String {
        ArtifactPathSerializer::rel_to_slash(&PathBuf::from(prefix).join(rel))
    }

    pub(super) fn is_declaration_file(path: &Path) -> bool {
        arkdown_path::SourceFileName::new(path).is_declaration()
    }
}

pub(super) struct ProfileResourceFileName<'a> {
    profile_name: &'a str,
}

impl<'a> ProfileResourceFileName<'a> {
    pub(super) fn new(profile_name: &'a str) -> Self {
        Self { profile_name }
    }

    pub(super) fn file_name(&self) -> PathBuf {
        PathBuf::from(self.profile_name).with_extension("json")
    }
}

struct ProfileGuidedApFileName<'a> {
    module_name: &'a str,
}

impl<'a> ProfileGuidedApFileName<'a> {
    fn new(module_name: &'a str) -> Self {
        Self { module_name }
    }

    fn file_name(&self) -> String {
        ArtifactPath::from_slash(self.module_name)
            .with_extension("ap")
            .into_string()
    }
}

pub(super) fn add_json_resource_stored<W: std::io::Write + std::io::Seek>(
    writer: &mut zip_writer::RawZipWriter<W>,
    archive_path: &str,
    file_path: &std::path::Path,
) -> Result<(), PackError> {
    let bytes = fs_err::read(file_path)?;
    let normalized = PackageToolJson::from_bytes(bytes).into_bytes();
    zip_writer::add_bytes_stored(writer, archive_path, &normalized)
}

/// JSON normalization performed by `app_packing_tool.jar` for ordinary JSON
/// inputs. The SDK parses them into Fastjson `JSONObject`, whose default
/// backing map is `java.util.HashMap(16)`, and serializes that map without the
/// ordered-field feature. Reproducing HashMap bucket iteration keeps packaged
/// JSON byte-compatible without encoding schema field lists.
pub(super) struct PackageToolJson {
    bytes: Vec<u8>,
}

impl PackageToolJson {
    pub(super) fn read(path: &Path) -> Result<Self, PackError> {
        Ok(Self {
            bytes: fs_err::read(path)?,
        })
    }

    pub(super) fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&self.bytes) else {
            return self.bytes;
        };
        serde_json::to_vec(&Self::into_fastjson_order(value))
            .expect("serializing parsed JSON should not fail")
    }

    fn into_fastjson_order(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(object) => {
                let capacity = Self::java_hash_map_capacity(object.len());
                let mut entries = object.into_iter().collect::<Vec<_>>();
                entries.sort_by_key(|(key, _)| Self::java_hash_bucket(key, capacity));
                let mut ordered = serde_json::Map::with_capacity(entries.len());
                for (key, value) in entries {
                    ordered.insert(key, Self::into_fastjson_order(value));
                }
                serde_json::Value::Object(ordered)
            }
            serde_json::Value::Array(values) => serde_json::Value::Array(
                values.into_iter().map(Self::into_fastjson_order).collect(),
            ),
            value => value,
        }
    }

    fn java_hash_map_capacity(size: usize) -> u32 {
        const MAXIMUM_CAPACITY: u32 = 1 << 30;
        let mut capacity = 16_u32;
        while size > (capacity as usize * 3 / 4) && capacity < MAXIMUM_CAPACITY {
            capacity *= 2;
        }
        capacity
    }

    fn java_hash_bucket(key: &str, capacity: u32) -> u32 {
        let hash = key.encode_utf16().fold(0_u32, |hash, code_unit| {
            hash.wrapping_mul(31).wrapping_add(u32::from(code_unit))
        });
        let spread = hash ^ (hash >> 16);
        spread & (capacity - 1)
    }
}

pub(super) fn compatible_api_version(version: &str) -> Option<u32> {
    let open = version.rfind('(')?;
    let close = version.rfind(')')?;
    if close <= open {
        return None;
    }
    version[open + 1..close].parse().ok()
}
