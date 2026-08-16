use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

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

  pub(crate) fn flush(&self) -> io::Result<()> {
    let mut state = self
      .inner
      .lock()
      .map_err(|_| io::Error::other("build log writer lock was poisoned"))?;
    if let Some(file) = state.file.as_mut() {
      file.flush()?;
    }
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

pub(crate) struct RuntimeArtifactStore {
  cache_dir: PathBuf,
  build_id: Arc<str>,
}

impl RuntimeArtifactStore {
  pub(crate) fn new(cache_dir: PathBuf, build_id: Arc<str>) -> io::Result<Self> {
    if build_id.is_empty()
      || !build_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "runtime artifact build id must contain only ASCII letters, digits, '-' or '_'",
      ));
    }
    Ok(Self {
      cache_dir,
      build_id,
    })
  }

  pub(crate) fn persist_report(
    &self,
    report_json: &str,
    analyze_html: bool,
  ) -> io::Result<PathBuf> {
    let report = Self::parse_json(report_json, "runtime report")?;
    let directory = self.cache_dir.join("report");
    fs::create_dir_all(&directory)?;
    self.remove_unusable_reports(&directory, analyze_html)?;
    self.rotate_reports(&directory)?;
    let base_name = format!("report-{}", self.build_id);
    let report_path = directory.join(format!("{base_name}.json"));
    Self::write_pretty_json(&report_path, &report)?;
    if analyze_html {
      self.persist_report_html(&directory, &base_name, &report)?;
    }
    Ok(report_path)
  }

  pub(crate) fn persist_details(&self, details_json: &str) -> io::Result<PathBuf> {
    let details = Self::parse_json(details_json, "runtime trace details")?;
    let path = self
      .cache_dir
      .join("outputs")
      .join("logs")
      .join("details")
      .join("details.json");
    Self::write_pretty_json(&path, &details)?;
    Ok(path)
  }

  pub(crate) fn persist_monitor(&self, monitor_json: &str) -> io::Result<PathBuf> {
    let monitor = Self::parse_json(monitor_json, "runtime process monitor")?;
    let path = self
      .cache_dir
      .join("report")
      .join(format!("report-monitor-{}.json", self.build_id));
    Self::write_pretty_json(&path, &monitor)?;
    Ok(path)
  }

  fn parse_json(value: &str, label: &str) -> io::Result<serde_json::Value> {
    serde_json::from_str(value).map_err(|error| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        format!("invalid {label} JSON: {error}"),
      )
    })
  }

  fn write_pretty_json(path: &Path, value: &serde_json::Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(path, bytes)
  }

  fn remove_unusable_reports(&self, directory: &Path, analyze_html: bool) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
      let entry = entry?;
      let name = entry.file_name();
      let name = name.to_string_lossy();
      let retained = Self::is_report_json(&name)
        || (!analyze_html && Self::is_report_html(&name))
        || matches!(name.as_ref(), "htmlResource" | "upload");
      if retained {
        continue;
      }
      let metadata = entry.metadata()?;
      if metadata.is_dir() {
        fs::remove_dir_all(entry.path())?;
      } else {
        fs::remove_file(entry.path())?;
      }
    }
    Ok(())
  }

  fn rotate_reports(&self, directory: &Path) -> io::Result<()> {
    let mut reports = fs::read_dir(directory)?
      .filter_map(Result::ok)
      .filter(|entry| Self::is_report_json(&entry.file_name().to_string_lossy()))
      .map(|entry| {
        let timestamp = entry
          .metadata()
          .ok()
          .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()).ok())
          .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
          .unwrap_or_default();
        (timestamp, entry.path())
      })
      .collect::<Vec<_>>();
    reports.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    for (_, stale) in reports.into_iter().skip(9) {
      fs::remove_file(stale)?;
    }
    Ok(())
  }

  fn persist_report_html(
    &self,
    directory: &Path,
    base_name: &str,
    report: &serde_json::Value,
  ) -> io::Result<()> {
    let payload = serde_json::to_string(&serde_json::to_string(report).map_err(io::Error::other)?)
      .map_err(io::Error::other)?
      .replace("</", "<\\/");
    let html = format!(
      "<!doctype html><html><head><meta charset=\"utf-8\"><title>Hvigor report</title></head><body><div id=\"root\"></div><script>window.__HVIGOR_REPORT__ = {payload};</script></body></html>"
    );
    fs::write(directory.join(format!("{base_name}.html")), html)
  }

  fn is_report_json(name: &str) -> bool {
    name
      .strip_prefix("report-")
      .and_then(|value| value.strip_suffix(".json"))
      .is_some_and(|value| {
        let build_id = value.strip_prefix("monitor-").unwrap_or(value);
        !build_id.is_empty() && build_id.chars().all(|character| character.is_ascii_digit())
      })
  }

  fn is_report_html(name: &str) -> bool {
    name
      .strip_prefix("report")
      .and_then(|value| value.strip_suffix(".html"))
      .is_some_and(|value| {
        let build_id = value.strip_prefix('-').unwrap_or(value);
        build_id.chars().all(|character| character.is_ascii_digit())
      })
  }
}
