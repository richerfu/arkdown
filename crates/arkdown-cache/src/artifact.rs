//! Content-backed stage completion records. Published only after successful output.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oxc_index::IndexVec;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CacheError, CacheFingerprint};

const SCHEMA: u32 = 1;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

oxc_index::define_nonmax_u32_index_type! { struct ArtifactInputId; }

struct ArtifactFileInput {
    path: Vec<u8>,
    digest: Option<[u8; 32]>,
}

struct ArtifactFileReader {
    buffer: Box<[u8]>,
}

impl ArtifactFileReader {
    fn new() -> Self {
        Self {
            buffer: vec![0; 64 * 1024].into_boxed_slice(),
        }
    }

    fn read(&mut self, path: &Path) -> Result<ArtifactFileInput, CacheError> {
        let identity = serde_json::to_vec(path)?;
        let mut file = match fs_err::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ArtifactFileInput {
                    path: identity,
                    digest: None,
                });
            }
            Err(error) => return Err(error.into()),
        };
        let mut content = Sha256::new();
        loop {
            let count = file.read(&mut self.buffer)?;
            if count == 0 {
                break;
            }
            content.update(&self.buffer[..count]);
        }
        Ok(ArtifactFileInput {
            path: identity,
            digest: Some(content.finalize().into()),
        })
    }
}

/// Hash an explicit domain input inventory without retaining binary payloads.
pub struct ArtifactInputHasher {
    hash: Sha256,
    reader: ArtifactFileReader,
}

impl ArtifactInputHasher {
    pub fn new(domain: &str) -> Self {
        let mut result = Self {
            hash: Sha256::new(),
            reader: ArtifactFileReader::new(),
        };
        result.value(domain.as_bytes());
        result
    }

    pub fn value(&mut self, bytes: &[u8]) {
        self.hash.update((bytes.len() as u64).to_le_bytes());
        self.hash.update(bytes);
    }

    pub fn file(&mut self, path: &Path) -> Result<(), CacheError> {
        let input = self.reader.read(path)?;
        self.append_file(&input);
        Ok(())
    }

    /// Independent content reads run in parallel; merging by dense ID keeps
    /// exactly the same fingerprint as the serial file inventory.
    pub fn files(&mut self, paths: &[&Path]) -> Result<(), CacheError> {
        let paths: IndexVec<ArtifactInputId, &Path> = paths.iter().copied().collect();
        let jobs = paths.iter_enumerated().collect::<Vec<_>>();
        let mut inputs = jobs
            .into_par_iter()
            .map_init(ArtifactFileReader::new, |reader, (id, path)| {
                (id, reader.read(path))
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|(id, _)| *id);
        for (_, input) in inputs {
            self.append_file(&input?);
        }
        Ok(())
    }

    fn append_file(&mut self, input: &ArtifactFileInput) {
        self.value(&input.path);
        match &input.digest {
            Some(digest) => {
                self.value(b"present");
                self.value(digest);
            }
            None => self.value(b"missing"),
        }
    }

    pub fn finish(self) -> String {
        format!("{:x}", self.hash.finalize())
    }
}

#[derive(Serialize, Deserialize)]
struct ArtifactRecord {
    schema: u32,
    input: String,
    outputs: Vec<ArtifactOutput>,
}

#[derive(Serialize, Deserialize)]
struct ArtifactOutput {
    path: PathBuf,
    digest: String,
}

/// Cache owner shared by compile, archive and signing domains. Output content
/// is checked on every hit, including same-length corruption and deletions.
pub struct ArtifactCacheStore {
    path: PathBuf,
}

impl ArtifactCacheStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_output(directory: &Path, domain: &str, output: &Path) -> Result<Self, CacheError> {
        let identity = CacheFingerprint::content_hash_bytes(&serde_json::to_vec(output)?);
        Ok(Self::new(
            directory.join(format!("{domain}-{identity}.json")),
        ))
    }

    pub fn matches(&self, input: &str, outputs: &[PathBuf]) -> Result<bool, CacheError> {
        if outputs.is_empty() {
            return Ok(false);
        }
        match fs_err::metadata(&self.path) {
            Ok(metadata) if metadata.len() > 32 * 1024 * 1024 => return Ok(false),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        }
        let bytes = match fs_err::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let record: ArtifactRecord = match serde_json::from_slice(&bytes) {
            Ok(record) => record,
            Err(_) => return Ok(false),
        };
        if record.schema != SCHEMA || record.input != input || record.outputs.len() != outputs.len()
        {
            return Ok(false);
        }
        let checked = record
            .outputs
            .par_iter()
            .zip(outputs.par_iter())
            .enumerate()
            .map(|(index, (stored, path))| {
                let id = ArtifactInputId::new(index);
                let result = (|| {
                    if stored.path != *path || !path.is_file() {
                        return Ok(false);
                    }
                    let mut hash = ArtifactInputHasher::new("output");
                    hash.file(path)?;
                    Ok::<bool, CacheError>(hash.finish() == stored.digest)
                })();
                (id, result)
            })
            .collect::<Vec<_>>();
        // Indexed parallel collection preserves input order, including which
        // filesystem diagnostic wins when multiple outputs are unreadable.
        for (_, result) in checked {
            if !result? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn invalidate(&self) -> Result<(), CacheError> {
        match fs_err::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn commit(&self, input: &str, paths: &[PathBuf]) -> Result<(), CacheError> {
        if paths.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "stage completion requires actual outputs",
            )
            .into());
        }
        let captured = paths
            .par_iter()
            .enumerate()
            .map(|(index, path)| {
                let id = ArtifactInputId::new(index);
                let result = (|| {
                    if !path.is_file() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("stage output is missing: {}", path.display()),
                        )
                        .into());
                    }
                    let mut hash = ArtifactInputHasher::new("output");
                    hash.file(path)?;
                    Ok::<_, CacheError>(ArtifactOutput {
                        path: path.clone(),
                        digest: hash.finish(),
                    })
                })();
                (id, result)
            })
            .collect::<Vec<_>>();
        let outputs = captured
            .into_iter()
            .map(|(_, output)| output)
            .collect::<Result<Vec<_>, _>>()?;
        let record = ArtifactRecord {
            schema: SCHEMA,
            input: input.to_owned(),
            outputs,
        };
        let parent = self.path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "artifact cache has no parent")
        })?;
        fs_err::create_dir_all(parent)?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".artifact-{}-{sequence}.tmp", std::process::id()));
        fs_err::write(&temporary, serde_json::to_vec(&record)?)?;
        #[cfg(windows)]
        if self.path.is_file() {
            fs_err::remove_file(&self.path)?;
        }
        fs_err::rename(&temporary, &self.path)?;
        Ok(())
    }
}
