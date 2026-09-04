use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_LOG_SIZE: u64 = 2_097_152;
const BACKUPS: u32 = 9;

pub(crate) struct BuildLogWriter {
  inner: Mutex<BuildLogWriterState>,
}

struct BuildLogWriterState {
  path: Option<PathBuf>,
  file: Option<BufWriter<File>>,
  size: u64,
}

impl BuildLogWriter {
  pub(crate) fn new() -> Arc<Self> {
    Arc::new(Self {
      inner: Mutex::new(BuildLogWriterState {
        path: None,
        file: None,
        size: 0,
      }),
    })
  }

  pub(crate) fn reopen(&self, cache_dir: &Path) -> io::Result<()> {
    let directory = cache_dir.join("outputs").join("build-logs");
    fs::create_dir_all(&directory)?;
    let path = directory.join("build.log");
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let size = file.metadata()?.len();
    let mut state = self
      .inner
      .lock()
      .map_err(|_| io::Error::other("build log writer lock was poisoned"))?;
    if let Some(mut previous) = state.file.take() {
      let _ = previous.flush();
    }
    state.file = Some(BufWriter::new(file));
    state.path = Some(path);
    state.size = size;
    Ok(())
  }

  pub(crate) fn write_line(&self, line: &str) -> io::Result<()> {
    self.write_line_bytes(line.as_bytes())
  }

  pub(crate) fn write_line_bytes(&self, buf: &[u8]) -> io::Result<()> {
    if buf.is_empty() {
      return Ok(());
    }
    let mut state = self
      .inner
      .lock()
      .map_err(|_| io::Error::other("build log writer lock was poisoned"))?;
    let Some(path) = state.path.clone() else {
      return Ok(());
    };
    if state.size.saturating_add(buf.len() as u64) > MAX_LOG_SIZE {
      if let Some(mut previous) = state.file.take() {
        let _ = previous.flush();
      }
      Self::rotate_log(&path)?;
      let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
      state.file = Some(BufWriter::new(file));
      state.size = 0;
    }
    let written = buf.len() as u64;
    let flush_error = Self::looks_like_error(buf);
    if let Some(file) = state.file.as_mut() {
      file.write_all(buf)?;
      if flush_error {
        file.flush()?;
      }
    }
    state.size = state.size.saturating_add(written);
    Ok(())
  }

  fn rotate_log(path: &Path) -> io::Result<()> {
    let last = Self::backup_path(path, BACKUPS);
    if last.exists() {
      fs::remove_file(&last)?;
    }
    for index in (1..BACKUPS).rev() {
      let from = Self::backup_path(path, index);
      if from.exists() {
        fs::rename(from, Self::backup_path(path, index + 1))?;
      }
    }
    if path.exists() {
      fs::rename(path, Self::backup_path(path, 1))?;
    }
    Ok(())
  }

  fn backup_path(path: &Path, index: u32) -> PathBuf {
    let mut file_name = path
      .file_name()
      .map(|name| name.to_os_string())
      .unwrap_or_else(|| "build.log".into());
    file_name.push(format!(".{index}"));
    path.with_file_name(file_name)
  }

  fn looks_like_error(buf: &[u8]) -> bool {
    buf.windows(7).any(|window| window == b" ERROR ")
  }
}
