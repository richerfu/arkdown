//! Upstream-compatible ABC ProgramCache/protoBin support.
//!
//! `es2abc --cache-file` is a pandasm `Program` cache:
//!
//! - bare `--cache-file file.protoBin` maps the current source file to one
//!   protobuf `ProgramCache` file;
//! - `--cache-file @modules.cache` reads a semicolon manifest where column 1 is
//!   `SourceFile::fileName` and column 2 is the `.protoBin` path;
//! - source cache validity is `GetHash32String(source + fileName + recordName +
//!   sourcefile + pkgName + targetPandaFileVersion)`;
//! - ABC input cache validity is `GetHash32(abcBytes + fileName + recordName +
//!   sourcefile + pkgName + targetPandaFileVersion)`;
//! - a hit restores a `Program` into `progsInfo_`; final ABC emission still
//!   happens normally from all programs.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use prost::Message;

use crate::proto_panda::{abc_proto_programs_cache, AbcProtoProgramsCache, Program, ProgramCache};

const MURMUR_DEFAULT_SEED: u32 = 0x1234_5678;
const MURMUR_C1: u32 = 0xCC9E_2D51;
const MURMUR_C2: u32 = 0x1B87_3593;
const MURMUR_MAIN_CONSTANT: u32 = 0xE654_6B64;
const MURMUR_FINALIZE_FIRST_MULTIPLIER: u32 = 0x85EB_CA6B;
const MURMUR_FINALIZE_SECOND_MULTIPLIER: u32 = 0x0C2B_AE35;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheManifestEntry {
    pub source_file: String,
    pub proto_bin: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheFileMap {
    entries: Vec<CacheManifestEntry>,
}

impl CacheFileMap {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn from_entries(entries: Vec<CacheManifestEntry>) -> Self {
        Self { entries }
    }

    pub fn parse_option(cache_input: &str, source_file: &str) -> io::Result<Self> {
        if let Some(manifest) = cache_input.strip_prefix('@') {
            return Self::read_manifest(Path::new(manifest));
        }

        Ok(Self::from_entries(vec![CacheManifestEntry {
            source_file: source_file.to_string(),
            proto_bin: PathBuf::from(cache_input),
        }]))
    }

    pub fn read_manifest(path: &Path) -> io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let entries = content
            .lines()
            .filter_map(parse_manifest_line)
            .collect::<Vec<_>>();
        Ok(Self::from_entries(entries))
    }

    pub fn get(&self, source_file: &str) -> Option<&Path> {
        self.entries
            .iter()
            .find(|entry| entry.source_file == source_file)
            .map(|entry| entry.proto_bin.as_path())
    }

    pub fn entries(&self) -> &[CacheManifestEntry] {
        &self.entries
    }
}

fn parse_manifest_line(line: &str) -> Option<CacheManifestEntry> {
    let items = line
        .split(';')
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if items.len() != 2 {
        return None;
    }
    Some(CacheManifestEntry {
        source_file: items[0].to_string(),
        proto_bin: PathBuf::from(items[1]),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramCacheEntry {
    pub hash_code: u32,
    pub program: Program,
}

pub struct ProgramCacheFile;

impl ProgramCacheFile {
    pub fn read(path: &Path) -> io::Result<Option<ProgramCacheEntry>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let proto = match ProgramCache::decode(bytes.as_slice()) {
            Ok(proto) => proto,
            Err(_) => return Ok(None),
        };
        let Some(program) = proto.program else {
            return Ok(None);
        };
        Ok(Some(ProgramCacheEntry {
            hash_code: proto.hash_code,
            program,
        }))
    }

    pub fn read_if_hash_matches(path: &Path, hash_code: u32) -> io::Result<Option<Program>> {
        let Some(entry) = Self::read(path)? else {
            return Ok(None);
        };
        if entry.hash_code == hash_code {
            Ok(Some(entry.program))
        } else {
            Ok(None)
        }
    }

    pub fn write(path: &Path, hash_code: u32, program: Program) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let proto = ProgramCache {
            hash_code,
            program: Some(program),
        };
        std::fs::write(path, proto.encode_to_vec())
    }
}

pub struct ProgramProtoFile;

impl ProgramProtoFile {
    pub fn read(path: &Path) -> io::Result<Option<Program>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match Program::decode(bytes.as_slice()) {
            Ok(program) => Ok(Some(program)),
            Err(_) => Ok(None),
        }
    }

    pub fn write(path: &Path, program: Program) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, program.encode_to_vec())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbcProgramsCacheEntry {
    pub hash_code: u32,
    pub programs: Vec<(String, Program)>,
}

pub struct AbcProgramsCacheFile;

impl AbcProgramsCacheFile {
    pub fn read(path: &Path) -> io::Result<Option<AbcProgramsCacheEntry>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let proto = match AbcProtoProgramsCache::decode(bytes.as_slice()) {
            Ok(proto) => proto,
            Err(_) => return Ok(None),
        };
        let mut programs = BTreeMap::new();
        for item in proto.abc_proto_programs_map {
            let Some(program) = item.program else {
                continue;
            };
            programs.entry(item.name).or_insert(program);
        }
        Ok(Some(AbcProgramsCacheEntry {
            hash_code: proto.hash_code,
            programs: programs.into_iter().collect(),
        }))
    }

    pub fn read_if_hash_matches(
        path: &Path,
        hash_code: u32,
    ) -> io::Result<Option<Vec<(String, Program)>>> {
        let Some(entry) = Self::read(path)? else {
            return Ok(None);
        };
        if entry.hash_code == hash_code {
            Ok(Some(entry.programs))
        } else {
            Ok(None)
        }
    }

    pub fn write(path: &Path, hash_code: u32, programs: Vec<(String, Program)>) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut sorted_programs = BTreeMap::new();
        for (name, program) in programs {
            sorted_programs.entry(name).or_insert(program);
        }
        let proto = AbcProtoProgramsCache {
            hash_code,
            abc_proto_programs_map: sorted_programs
                .into_iter()
                .map(
                    |(name, program)| abc_proto_programs_cache::AbcProtoProgramCache {
                        name,
                        program: Some(program),
                    },
                )
                .collect(),
        };
        std::fs::write(path, proto.encode_to_vec())
    }
}

pub fn source_cache_hash(
    source: &str,
    file_name: &str,
    record_name: &str,
    source_file: &str,
    package_name: &str,
    panda_file_version: u32,
    target_api_version: u32,
) -> u32 {
    let version = panda_file_version_string(panda_file_version);
    let target_api = target_api_version.to_string();
    let mut bytes = Vec::with_capacity(
        source.len()
            + file_name.len()
            + record_name.len()
            + source_file.len()
            + package_name.len()
            + version.len()
            + target_api.len(),
    );
    bytes.extend_from_slice(source.as_bytes());
    bytes.extend_from_slice(file_name.as_bytes());
    bytes.extend_from_slice(record_name.as_bytes());
    bytes.extend_from_slice(source_file.as_bytes());
    bytes.extend_from_slice(package_name.as_bytes());
    bytes.extend_from_slice(version.as_bytes());
    bytes.extend_from_slice(target_api.as_bytes());
    get_hash32_string(&bytes)
}

pub fn abc_input_cache_hash(
    abc_bytes: &[u8],
    file_name: &str,
    record_name: &str,
    source_file: &str,
    package_name: &str,
    panda_file_version: u32,
) -> u32 {
    let version = panda_file_version_string(panda_file_version);
    let mut bytes = Vec::with_capacity(
        abc_bytes.len()
            + file_name.len()
            + record_name.len()
            + source_file.len()
            + package_name.len()
            + version.len(),
    );
    bytes.extend_from_slice(abc_bytes);
    bytes.extend_from_slice(file_name.as_bytes());
    bytes.extend_from_slice(record_name.as_bytes());
    bytes.extend_from_slice(source_file.as_bytes());
    bytes.extend_from_slice(package_name.as_bytes());
    bytes.extend_from_slice(version.as_bytes());
    get_hash32(&bytes)
}

pub(crate) fn panda_file_version_string(version: u32) -> String {
    let [major, minor, feature, build] = version.to_le_bytes();
    format!("{major}.{minor}.{feature}.{build}")
}

pub fn get_hash32(bytes: &[u8]) -> u32 {
    murmur_hash32(bytes, MURMUR_DEFAULT_SEED)
}

pub fn get_hash32_string(bytes: &[u8]) -> u32 {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    murmur_hash32(&bytes[..end], MURMUR_DEFAULT_SEED)
}

fn murmur_hash32(bytes: &[u8], seed: u32) -> u32 {
    let mut hash = seed;
    let mut chunks = bytes.chunks_exact(4);
    for chunk in &mut chunks {
        let mut k1 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k1 = k1.wrapping_mul(MURMUR_C1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(MURMUR_C2);

        hash ^= k1;
        hash = hash.rotate_left(13);
        hash = hash.wrapping_mul(5).wrapping_add(MURMUR_MAIN_CONSTANT);
    }

    let tail = chunks.remainder();
    let mut k1 = 0u32;
    for i in (0..tail.len()).rev() {
        k1 ^= (tail[i] as u32) << (8 * i);
    }
    if !tail.is_empty() {
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(MURMUR_C2);
        hash ^= k1;
    }

    hash ^= bytes.len() as u32;
    finalize_murmur_hash32(hash)
}

fn finalize_murmur_hash32(mut hash: u32) -> u32 {
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(MURMUR_FINALIZE_FIRST_MULTIPLIER);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(MURMUR_FINALIZE_SECOND_MULTIPLIER);
    hash ^= hash >> 16;
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto_panda::{program, Record};

    #[test]
    fn upstream_murmur_hash_matches_known_values() {
        assert_eq!(get_hash32_string(b""), 0x453c_77fc);
        assert_eq!(get_hash32_string(b"a"), 0x2444_ff8e);
        assert_eq!(get_hash32_string(b"abc"), 0x647e_bde0);
        assert_eq!(
            source_cache_hash(
                "export const x = 1;",
                "entry/main",
                "entry/main",
                "main.ts",
                "entry",
                0x0001_000D,
                20,
            ),
            0x5ae6_2daa
        );
    }

    #[test]
    fn cache_hash_uses_target_panda_file_version() {
        assert_eq!(panda_file_version_string(0x0002_000C), "12.0.2.0");
        assert_eq!(panda_file_version_string(0x0006_000C), "12.0.6.0");
        assert_eq!(panda_file_version_string(0x0001_000D), "13.0.1.0");

        let api12_hash = source_cache_hash(
            "export const x = 1;",
            "entry/main",
            "entry/main",
            "main.ts",
            "entry",
            0x0002_000C,
            12,
        );
        let api13_hash = source_cache_hash(
            "export const x = 1;",
            "entry/main",
            "entry/main",
            "main.ts",
            "entry",
            0x0001_000D,
            13,
        );
        assert_ne!(api12_hash, api13_hash);
    }

    #[test]
    fn cache_hash_uses_target_api_even_when_panda_version_matches() {
        let api20_hash = source_cache_hash(
            "export const x = 1;",
            "entry/main",
            "entry/main",
            "main.ts",
            "entry",
            0x0001_000D,
            20,
        );
        let api21_hash = source_cache_hash(
            "export const x = 1;",
            "entry/main",
            "entry/main",
            "main.ts",
            "entry",
            0x0001_000D,
            21,
        );

        assert_ne!(api20_hash, api21_hash);
    }

    #[test]
    fn manifest_uses_source_file_to_proto_bin_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("modules.cache");
        std::fs::write(
            &manifest,
            "a.ts;/tmp/a.protoBin\nignored\nb.ts;/tmp/b.protoBin;extra\nc.ts;/tmp/c.protoBin\n",
        )
        .unwrap();

        let map = CacheFileMap::read_manifest(&manifest).unwrap();
        assert_eq!(map.entries().len(), 2);
        assert_eq!(map.get("a.ts"), Some(Path::new("/tmp/a.protoBin")));
        assert_eq!(map.get("c.ts"), Some(Path::new("/tmp/c.protoBin")));
        assert!(map.get("b.ts").is_none());
    }

    #[test]
    fn program_cache_proto_bin_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("entry.protoBin");
        let program = Program {
            lang: 0,
            record_table: vec![program::RecordTable {
                key: b"entry/main".to_vec(),
                value: Some(Record {
                    name: b"entry/main".to_vec(),
                    language: 0,
                    body_presence: true,
                    ..Default::default()
                }),
            }],
            ..Default::default()
        };

        ProgramCacheFile::write(&path, 0x1234, program.clone()).unwrap();
        let hit = ProgramCacheFile::read_if_hash_matches(&path, 0x1234)
            .unwrap()
            .unwrap();
        assert_eq!(hit, program);
        assert!(ProgramCacheFile::read_if_hash_matches(&path, 0x5678)
            .unwrap()
            .is_none());
    }

    #[test]
    fn abc_programs_cache_proto_bin_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input.protoBin");
        let program_a = Program {
            lang: 1,
            record_table: vec![program::RecordTable {
                key: b"La;".to_vec(),
                value: Some(Record {
                    name: b"La;".to_vec(),
                    language: 0,
                    body_presence: true,
                    ..Default::default()
                }),
            }],
            ..Default::default()
        };
        let program_z = Program {
            lang: 2,
            record_table: vec![program::RecordTable {
                key: b"Lz;".to_vec(),
                value: Some(Record {
                    name: b"Lz;".to_vec(),
                    language: 0,
                    body_presence: true,
                    ..Default::default()
                }),
            }],
            ..Default::default()
        };
        let programs = vec![
            ("input.abc|Lz;".to_string(), program_z.clone()),
            ("input.abc|La;".to_string(), program_a.clone()),
        ];

        AbcProgramsCacheFile::write(&path, 0x1234, programs.clone()).unwrap();
        let hit = AbcProgramsCacheFile::read_if_hash_matches(&path, 0x1234)
            .unwrap()
            .unwrap();
        assert_eq!(
            hit,
            vec![
                ("input.abc|La;".to_string(), program_a),
                ("input.abc|Lz;".to_string(), program_z),
            ]
        );
        assert!(AbcProgramsCacheFile::read_if_hash_matches(&path, 0x5678)
            .unwrap()
            .is_none());
    }
}
