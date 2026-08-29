use std::path::{Component, Path, PathBuf};

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use serde_json::Value;
use sha2::Digest;

/// Rust-owned filesystem boundary for the formally declared Hvigor
/// `FileUtil` surface. JavaScript only unwraps `NormalizedFile.filePath` and
/// preserves the public static method names.
#[napi]
#[derive(Default)]
pub struct RuntimeHvigorFileApi;

#[napi]
impl RuntimeHvigorFileApi {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self
  }

  #[napi]
  pub fn exists(&self, path: String) -> bool {
    Path::new(&path).exists()
  }

  #[napi]
  pub fn is_directory(&self, path: String) -> bool {
    Path::new(&path).is_dir()
  }

  #[napi]
  pub fn is_file(&self, path: String) -> bool {
    Path::new(&path).is_file()
  }

  #[napi]
  pub fn check_copy_path_is_same(&self, source: String, target: String) -> bool {
    let Ok(source) = fs_err::metadata(source) else {
      return false;
    };
    let Ok(target) = fs_err::metadata(target) else {
      return false;
    };
    Self::metadata_is_identical(&source, &target)
  }

  #[napi]
  pub fn ensure_directory(&self, path: String) -> napi::Result<()> {
    fs_err::create_dir_all(path).map_err(Self::io_error)
  }

  #[napi]
  pub fn ensure_file(&self, path: String) -> napi::Result<()> {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent).map_err(Self::io_error)?;
    }
    if !path.exists() {
      fs_err::File::create(path).map_err(Self::io_error)?;
    }
    Ok(())
  }

  #[napi]
  pub fn read_file_sync(&self, path: String) -> napi::Result<Buffer> {
    fs_err::read(path).map(Buffer::from).map_err(Self::io_error)
  }

  #[napi]
  pub async fn read_file(&self, path: String) -> napi::Result<Buffer> {
    tokio::fs::read(path)
      .await
      .map(Buffer::from)
      .map_err(Self::io_error)
  }

  #[napi]
  pub fn read_json5(&self, path: String) -> napi::Result<Value> {
    let source = fs_err::read_to_string(path).map_err(Self::io_error)?;
    json5::from_str(&source)
      .map_err(|error| napi::Error::from_reason(format!("failed to parse JSON5: {error}")))
  }

  #[napi]
  pub fn hash_text(&self, content: String) -> String {
    format!("{:x}", sha2::Sha256::digest(content.as_bytes()))
  }

  #[napi]
  pub fn write_file_sync(&self, path: String, content: Buffer) -> napi::Result<()> {
    fs_err::write(path, content.as_ref()).map_err(Self::io_error)
  }

  #[napi]
  pub async fn write_file(&self, path: String, content: Buffer) -> napi::Result<()> {
    tokio::fs::write(path, content.as_ref())
      .await
      .map_err(Self::io_error)
  }

  #[napi]
  pub fn copy_file_sync(&self, source: String, target: String) -> napi::Result<()> {
    fs_err::copy(source, target)
      .map(|_| ())
      .map_err(Self::io_error)
  }

  #[napi]
  pub async fn copy_file(&self, source: String, target: String) -> napi::Result<()> {
    tokio::fs::copy(source, target)
      .await
      .map(|_| ())
      .map_err(Self::io_error)
  }

  #[napi]
  pub fn path_resolve(&self, segments: Vec<String>) -> napi::Result<String> {
    let mut resolved = std::env::current_dir().map_err(Self::io_error)?;
    for segment in segments {
      resolved.push(segment);
    }
    Ok(
      Self::normalize_path(&resolved)
        .to_string_lossy()
        .into_owned(),
    )
  }
}

impl RuntimeHvigorFileApi {
  fn io_error(error: std::io::Error) -> napi::Error {
    napi::Error::from_reason(error.to_string())
  }

  fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
      match component {
        Component::CurDir => {}
        Component::ParentDir => {
          normalized.pop();
        }
        Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
          normalized.push(component.as_os_str());
        }
      }
    }
    normalized
  }

  #[cfg(unix)]
  fn metadata_is_identical(source: &std::fs::Metadata, target: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    target.ino() != 0
      && target.dev() != 0
      && source.ino() == target.ino()
      && source.dev() == target.dev()
  }

  #[cfg(windows)]
  fn metadata_is_identical(source: &std::fs::Metadata, target: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    source.file_index() == target.file_index()
      && source.volume_serial_number() == target.volume_serial_number()
  }

  #[cfg(not(any(unix, windows)))]
  fn metadata_is_identical(_source: &std::fs::Metadata, _target: &std::fs::Metadata) -> bool {
    false
  }
}
