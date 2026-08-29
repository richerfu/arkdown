use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use tracing::metadata::LevelFilter;
use tracing::span;
use tracing::subscriber::Interest;
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use super::emit::RuntimeLogRecord;
use super::persist::{BuildLogWriter, RuntimeArtifactStore};

static GLOBAL: OnceLock<Arc<RuntimeObservability>> = OnceLock::new();
static SUBSCRIBER_READY: OnceLock<()> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

const JS_OFF: &str = "arkdown_napi::hvigor=off,arkdown_napi::metric=off";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservabilityMode {
  Cli,
  Napi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsoleOwner {
  Log4js,
  Tracing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ObservabilityLevel {
  Error,
  Warn,
  Info,
  Debug,
}

impl ObservabilityLevel {
  pub(crate) fn parse(value: &str) -> Self {
    match value.trim().to_ascii_lowercase().as_str() {
      "error" | "err" => Self::Error,
      "warn" | "warning" => Self::Warn,
      "debug" | "trace" => Self::Debug,
      _ => Self::Info,
    }
  }

  pub(crate) fn as_str(self) -> &'static str {
    match self {
      Self::Error => "error",
      Self::Warn => "warn",
      Self::Info => "info",
      Self::Debug => "debug",
    }
  }

  pub(crate) fn to_tracing(self) -> Level {
    match self {
      Self::Error => Level::ERROR,
      Self::Warn => Level::WARN,
      Self::Info => Level::INFO,
      Self::Debug => Level::DEBUG,
    }
  }

  fn file_filter_spec(self) -> String {
    format!("arkdown={},arkdown_napi={}", self.as_str(), self.as_str())
  }
}

#[derive(Debug, Clone)]
pub(crate) struct ObservabilityInstallPlan {
  pub mode: ObservabilityMode,
  pub console_owner: ConsoleOwner,
  pub cache_dir: PathBuf,
  pub build_id: Arc<str>,
  pub log_level: ObservabilityLevel,
}

struct ObservabilityState {
  active: bool,
  source: ObservabilityInstallSource,
  mode: ObservabilityMode,
  console_owner: ConsoleOwner,
  build_id: Arc<str>,
  cache_dir: PathBuf,
  log_level: ObservabilityLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservabilityInstallSource {
  Explicit,
  SafetyNet,
}

pub struct RuntimeObservability {
  state: RwLock<ObservabilityState>,
  stderr_filter: Arc<RwLock<ObservabilityFilterState>>,
  file_filter: Arc<RwLock<ObservabilityFilterState>>,
  stderr_style: Arc<RwLock<FormatStyle>>,
  file_writer: Arc<BuildLogWriter>,
}

impl RuntimeObservability {
  pub fn install_cli() -> anyhow::Result<Arc<Self>> {
    let plan = ObservabilityInstallPlan {
      mode: ObservabilityMode::Cli,
      console_owner: ConsoleOwner::Tracing,
      cache_dir: PathBuf::new(),
      build_id: Arc::from("cli"),
      log_level: ObservabilityLevel::Info,
    };
    Self::install(plan, ObservabilityInstallSource::Explicit)
  }

  pub(crate) fn install_napi(plan: ObservabilityInstallPlan) -> anyhow::Result<Arc<Self>> {
    Self::install(plan, ObservabilityInstallSource::Explicit)
  }

  pub(crate) fn install_build_safety_net(config: &crate::BuildConfig) -> anyhow::Result<Arc<Self>> {
    if let Some(existing) =
      Self::global().filter(|observability| observability.has_active_napi_plan())
    {
      return Ok(existing);
    }
    let project = config
      .project
      .as_deref()
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from("."));
    Self::install(
      ObservabilityInstallPlan {
        mode: ObservabilityMode::Napi,
        console_owner: Self::console_owner_from_env(),
        cache_dir: config
          .hvigor_cache_dir
          .as_deref()
          .map(PathBuf::from)
          .unwrap_or_else(|| project.join(".hvigor")),
        build_id: Arc::from("napi-build"),
        log_level: ObservabilityLevel::Info,
      },
      ObservabilityInstallSource::SafetyNet,
    )
  }

  pub(crate) fn install_runtime_task_safety_net(seed: &crate::runtime_task::RuntimeTaskBridgeSeed) {
    if Self::global().is_some_and(|observability| observability.has_active_napi_plan()) {
      return;
    }
    let _ = Self::install(
      ObservabilityInstallPlan {
        mode: ObservabilityMode::Napi,
        console_owner: Self::console_owner_from_env(),
        cache_dir: seed.project_cache_dir(),
        build_id: Arc::from("napi-runtime-task"),
        log_level: ObservabilityLevel::Info,
      },
      ObservabilityInstallSource::SafetyNet,
    );
  }

  pub fn global() -> Option<Arc<Self>> {
    GLOBAL.get().cloned()
  }

  pub fn build_id(&self) -> Arc<str> {
    self
      .state
      .read()
      .map(|state| Arc::clone(&state.build_id))
      .unwrap_or_else(|_| Arc::from(""))
  }

  /// Materialize the formally declared `BuildResult.getReportJson()` payload.
  /// Undocumented metric listeners are intentionally outside Arkdown's public
  /// compatibility contract, while the 6.24.3 report envelope remains stable.
  pub(crate) fn hvigor_report_snapshot(ppid: u32) -> serde_json::Value {
    serde_json::json!({
      "version": "2.0",
      "ppid": ppid,
      "events": [],
    })
  }

  pub(crate) fn has_active_napi_plan(&self) -> bool {
    self
      .state
      .read()
      .map(|state| {
        state.active
          && state.mode == ObservabilityMode::Napi
          && state.source == ObservabilityInstallSource::Explicit
      })
      .unwrap_or(false)
  }

  pub(crate) fn is_enabled(&self, level: ObservabilityLevel, target: &str) -> bool {
    // JavaScript emits only through these two static tracing targets. Ask the
    // installed subscriber for those exact callsites so span-aware EnvFilter
    // directives and later filter swaps stay authoritative. The lightweight
    // directive probe remains only for unknown diagnostic targets.
    match (target, level) {
      ("arkdown_napi::hvigor", ObservabilityLevel::Error) => {
        return tracing::enabled!(target: "arkdown_napi::hvigor", Level::ERROR);
      }
      ("arkdown_napi::hvigor", ObservabilityLevel::Warn) => {
        return tracing::enabled!(target: "arkdown_napi::hvigor", Level::WARN);
      }
      ("arkdown_napi::hvigor", ObservabilityLevel::Info) => {
        return tracing::enabled!(target: "arkdown_napi::hvigor", Level::INFO);
      }
      ("arkdown_napi::hvigor", ObservabilityLevel::Debug) => {
        return tracing::enabled!(target: "arkdown_napi::hvigor", Level::DEBUG);
      }
      ("arkdown_napi::metric", ObservabilityLevel::Error) => {
        return tracing::enabled!(target: "arkdown_napi::metric", Level::ERROR);
      }
      ("arkdown_napi::metric", ObservabilityLevel::Warn) => {
        return tracing::enabled!(target: "arkdown_napi::metric", Level::WARN);
      }
      ("arkdown_napi::metric", ObservabilityLevel::Info) => {
        return tracing::enabled!(target: "arkdown_napi::metric", Level::INFO);
      }
      ("arkdown_napi::metric", ObservabilityLevel::Debug) => {
        return tracing::enabled!(target: "arkdown_napi::metric", Level::DEBUG);
      }
      _ => {}
    }
    let tracing_level = level.to_tracing();
    self.file_filter_enabled(target, &tracing_level)
      || self.stderr_filter_enabled(target, &tracing_level)
  }

  pub fn emit_log(&self, record: &RuntimeLogRecord) {
    super::emit::RuntimeObservabilityEmitter::new(self).emit_log(record);
  }

  pub fn finish(&self) -> anyhow::Result<()> {
    let flush_result = self.file_writer.flush();
    self
      .state
      .write()
      .map_err(|_| anyhow::anyhow!("observability state lock was poisoned"))?
      .active = false;
    flush_result.map_err(Into::into)
  }

  pub fn persist_report(&self, report_json: &str, analyze_html: bool) -> anyhow::Result<PathBuf> {
    Ok(
      self
        .artifact_store()?
        .persist_report(report_json, analyze_html)?,
    )
  }

  pub fn persist_details(&self, details_json: &str) -> anyhow::Result<PathBuf> {
    Ok(self.artifact_store()?.persist_details(details_json)?)
  }

  pub fn persist_monitor(&self, monitor_json: &str) -> anyhow::Result<PathBuf> {
    Ok(self.artifact_store()?.persist_monitor(monitor_json)?)
  }

  pub fn reopen_file(&self, cache_dir: &Path) -> anyhow::Result<()> {
    if cache_dir.as_os_str().is_empty() {
      return Ok(());
    }
    self.file_writer.reopen(cache_dir)?;
    if let Ok(mut state) = self.state.write() {
      state.cache_dir = cache_dir.to_path_buf();
    }
    Ok(())
  }

  fn install(
    plan: ObservabilityInstallPlan,
    source: ObservabilityInstallSource,
  ) -> anyhow::Result<Arc<Self>> {
    let _install_guard = INSTALL_LOCK
      .lock()
      .map_err(|_| anyhow::anyhow!("observability install lock was poisoned"))?;
    let (stderr_spec, file_spec) = Self::filter_specs(&plan);
    if let Some(existing) = GLOBAL.get() {
      existing.apply_plan(&plan, source, &stderr_spec, &file_spec)?;
      return Ok(Arc::clone(existing));
    }

    let file_writer = BuildLogWriter::new();
    if plan.mode == ObservabilityMode::Napi && !plan.cache_dir.as_os_str().is_empty() {
      file_writer.reopen(&plan.cache_dir)?;
    }

    let installed = Arc::new(Self {
      state: RwLock::new(ObservabilityState {
        active: true,
        source,
        mode: plan.mode,
        console_owner: plan.console_owner,
        build_id: Arc::clone(&plan.build_id),
        cache_dir: plan.cache_dir.clone(),
        log_level: plan.log_level,
      }),
      stderr_filter: Arc::new(RwLock::new(ObservabilityFilterState::parse(&stderr_spec)?)),
      file_filter: Arc::new(RwLock::new(ObservabilityFilterState::parse(&file_spec)?)),
      stderr_style: Arc::new(RwLock::new(if plan.mode == ObservabilityMode::Cli {
        FormatStyle::Cli
      } else {
        FormatStyle::Structured
      })),
      file_writer: Arc::clone(&file_writer),
    });

    if SUBSCRIBER_READY.get().is_none() {
      let layer = ObservabilityLayer {
        stderr_filter: Arc::clone(&installed.stderr_filter),
        file_filter: Arc::clone(&installed.file_filter),
        stderr_style: Arc::clone(&installed.stderr_style),
        writer: Arc::clone(&file_writer),
      };
      tracing_subscriber::registry()
        .with(layer)
        .try_init()
        .map_err(|error| anyhow::anyhow!("install tracing subscriber: {error}"))?;
      let _ = SUBSCRIBER_READY.set(());
    }

    let _ = GLOBAL.set(Arc::clone(&installed));
    Ok(GLOBAL.get().cloned().unwrap_or(installed))
  }

  fn artifact_store(&self) -> anyhow::Result<RuntimeArtifactStore> {
    let state = self
      .state
      .read()
      .map_err(|_| anyhow::anyhow!("observability state lock was poisoned"))?;
    Ok(RuntimeArtifactStore::new(
      state.cache_dir.clone(),
      Arc::clone(&state.build_id),
    )?)
  }

  fn apply_plan(
    &self,
    plan: &ObservabilityInstallPlan,
    source: ObservabilityInstallSource,
    stderr_spec: &str,
    file_spec: &str,
  ) -> anyhow::Result<()> {
    {
      let mut state = self
        .state
        .write()
        .map_err(|_| anyhow::anyhow!("observability state lock was poisoned"))?;
      state.active = true;
      state.source = source;
      state.mode = plan.mode;
      state.console_owner = plan.console_owner;
      state.build_id = Arc::clone(&plan.build_id);
      state.log_level = plan.log_level;
    }
    self.swap_filter(&self.stderr_filter, stderr_spec)?;
    self.swap_filter(&self.file_filter, file_spec)?;
    *self
      .stderr_style
      .write()
      .map_err(|_| anyhow::anyhow!("observability stderr style lock was poisoned"))? =
      if plan.mode == ObservabilityMode::Cli {
        FormatStyle::Cli
      } else {
        FormatStyle::Structured
      };
    if plan.mode == ObservabilityMode::Napi && !plan.cache_dir.as_os_str().is_empty() {
      self.reopen_file(&plan.cache_dir)?;
    }
    Ok(())
  }

  fn swap_filter(&self, slot: &RwLock<ObservabilityFilterState>, spec: &str) -> anyhow::Result<()> {
    let mut filter = slot
      .write()
      .map_err(|_| anyhow::anyhow!("observability filter lock was poisoned"))?;
    *filter = ObservabilityFilterState::parse(spec)?;
    Ok(())
  }

  fn file_filter_enabled(&self, target: &str, level: &Level) -> bool {
    self
      .file_filter
      .read()
      .map(|filter| filter.probe.enabled(target, level))
      .unwrap_or(false)
  }

  fn stderr_filter_enabled(&self, target: &str, level: &Level) -> bool {
    self
      .stderr_filter
      .read()
      .map(|filter| filter.probe.enabled(target, level))
      .unwrap_or(false)
  }

  fn filter_specs(plan: &ObservabilityInstallPlan) -> (String, String) {
    match plan.mode {
      ObservabilityMode::Cli => Self::cli_filter_specs(),
      ObservabilityMode::Napi => Self::napi_filter_specs(plan),
    }
  }

  fn cli_filter_specs() -> (String, String) {
    match Self::env_log_spec() {
      Some(spec) => (spec.clone(), spec),
      None => (
        "info".to_string(),
        ObservabilityLevel::Info.file_filter_spec(),
      ),
    }
  }

  fn napi_filter_specs(plan: &ObservabilityInstallPlan) -> (String, String) {
    let env_spec = Self::env_log_spec();
    let console_tracing = plan.console_owner == ConsoleOwner::Tracing;
    match (console_tracing, env_spec.as_deref()) {
      (_, Some("off")) => ("off".to_string(), "off".to_string()),
      (false, None) => (
        format!("arkdown=warn,arkdown_napi=warn,{JS_OFF}"),
        plan.log_level.file_filter_spec(),
      ),
      (false, Some(spec)) => (Self::append_js_off(spec), spec.to_string()),
      (true, None) => {
        let spec = ObservabilityLevel::Info.file_filter_spec();
        (spec.clone(), spec)
      }
      (true, Some(spec)) => (spec.to_string(), spec.to_string()),
    }
  }

  fn env_log_spec() -> Option<String> {
    env::var("ARKDOWN_LOG")
      .ok()
      .or_else(|| env::var("RUST_LOG").ok())
      .map(|value| value.trim().to_string())
      .filter(|value| !value.is_empty())
  }

  fn append_js_off(spec: &str) -> String {
    format!("{spec},{JS_OFF}")
  }

  fn console_owner_from_env() -> ConsoleOwner {
    match env::var("ARKDOWN_LOG_CONSOLE") {
      Ok(value)
        if matches!(
          value.trim().to_ascii_lowercase().as_str(),
          "1" | "true" | "yes"
        ) =>
      {
        ConsoleOwner::Tracing
      }
      _ => ConsoleOwner::Log4js,
    }
  }
}

struct ObservabilityFilterState {
  env: EnvFilter,
  probe: ObservabilityDirectiveSet,
}

impl ObservabilityFilterState {
  fn parse(spec: &str) -> anyhow::Result<Self> {
    Ok(Self {
      env: EnvFilter::try_new(spec)
        .map_err(|error| anyhow::anyhow!("invalid observability filter {spec:?}: {error}"))?,
      probe: ObservabilityDirectiveSet::parse(spec),
    })
  }
}

#[derive(Debug, Clone)]
pub(crate) struct ObservabilityDirectiveSet {
  off: bool,
  default_level: Option<Level>,
  directives: Vec<(String, Option<Level>)>,
}

impl ObservabilityDirectiveSet {
  pub(crate) fn parse(spec: &str) -> Self {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("off") {
      return Self {
        off: true,
        default_level: None,
        directives: Vec::new(),
      };
    }
    let mut default_level = None;
    let mut directives = Vec::new();
    for part in spec.split(',') {
      let part = part.trim();
      if part.is_empty() {
        continue;
      }
      if let Some((target, level)) = part.split_once('=') {
        let parsed = parse_level_token(level);
        if target == "*" {
          default_level = parsed;
        } else {
          directives.retain(|(existing, _)| existing != target);
          directives.push((target.to_string(), parsed));
        }
      } else if let Some(level) = parse_level_token(part) {
        default_level = Some(level);
      }
    }
    directives.sort_by(|left, right| right.0.len().cmp(&left.0.len()));
    if default_level.is_none() && !directives.is_empty() {
      default_level = Some(Level::ERROR);
    }
    if default_level.is_none() && directives.is_empty() {
      default_level = Some(Level::INFO);
    }
    Self {
      off: false,
      default_level,
      directives,
    }
  }

  pub(crate) fn enabled(&self, target: &str, level: &Level) -> bool {
    if self.off {
      return false;
    }
    for (prefix, allowed) in &self.directives {
      if target_matches(target, prefix) {
        return match allowed {
          None => false,
          Some(maximum) => level <= maximum,
        };
      }
    }
    match self.default_level {
      Some(maximum) => level <= &maximum,
      None => false,
    }
  }
}

fn target_matches(target: &str, prefix: &str) -> bool {
  target == prefix
    || (target.starts_with(prefix) && target.as_bytes().get(prefix.len()).copied() == Some(b':'))
}

fn parse_level_token(value: &str) -> Option<Level> {
  match value.trim().to_ascii_lowercase().as_str() {
    "off" => None,
    "error" | "err" => Some(Level::ERROR),
    "warn" | "warning" => Some(Level::WARN),
    "info" => Some(Level::INFO),
    "debug" => Some(Level::DEBUG),
    "trace" => Some(Level::TRACE),
    _ => None,
  }
}

#[derive(Clone, Copy)]
enum FormatStyle {
  Cli,
  Structured,
}

struct ObservabilityLayer {
  stderr_filter: Arc<RwLock<ObservabilityFilterState>>,
  file_filter: Arc<RwLock<ObservabilityFilterState>>,
  stderr_style: Arc<RwLock<FormatStyle>>,
  writer: Arc<BuildLogWriter>,
}

impl<S> Layer<S> for ObservabilityLayer
where
  S: Subscriber,
{
  fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
    Interest::sometimes()
  }

  fn max_level_hint(&self) -> Option<LevelFilter> {
    Some(LevelFilter::TRACE)
  }

  fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
    self.stderr_enabled(metadata, ctx.clone()) || self.file_enabled(metadata, ctx)
  }

  fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
    if let Ok(filter) = self.stderr_filter.read() {
      filter.env.on_new_span(attrs, id, ctx.clone());
    }
    if let Ok(filter) = self.file_filter.read() {
      filter.env.on_new_span(attrs, id, ctx);
    }
  }

  fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
    if let Ok(filter) = self.stderr_filter.read() {
      filter.env.on_record(id, values, ctx.clone());
    }
    if let Ok(filter) = self.file_filter.read() {
      filter.env.on_record(id, values, ctx);
    }
  }

  fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
    if let Ok(filter) = self.stderr_filter.read() {
      filter.env.on_enter(id, ctx.clone());
    }
    if let Ok(filter) = self.file_filter.read() {
      filter.env.on_enter(id, ctx);
    }
  }

  fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
    if let Ok(filter) = self.stderr_filter.read() {
      filter.env.on_exit(id, ctx.clone());
    }
    if let Ok(filter) = self.file_filter.read() {
      filter.env.on_exit(id, ctx);
    }
  }

  fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
    if let Ok(filter) = self.stderr_filter.read() {
      filter.env.on_close(id.clone(), ctx.clone());
    }
    if let Ok(filter) = self.file_filter.read() {
      filter.env.on_close(id, ctx);
    }
  }

  fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
    if self.stderr_enabled(event.metadata(), ctx.clone()) {
      let style = self
        .stderr_style
        .read()
        .map(|style| *style)
        .unwrap_or(FormatStyle::Structured);
      let line = format_event_line(event, style);
      let _ = std::io::Write::write_all(&mut std::io::stderr(), line.as_bytes());
    }
    if self.file_enabled(event.metadata(), ctx) {
      let line = format_event_line(event, FormatStyle::Structured);
      let _ = self.writer.write_line(&line);
    }
  }
}

impl ObservabilityLayer {
  fn stderr_enabled<S>(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool
  where
    S: Subscriber,
  {
    self
      .stderr_filter
      .read()
      .map(|filter| filter.env.enabled(metadata, ctx))
      .unwrap_or(false)
  }

  fn file_enabled<S>(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool
  where
    S: Subscriber,
  {
    self
      .file_filter
      .read()
      .map(|filter| filter.env.enabled(metadata, ctx))
      .unwrap_or(false)
  }
}

fn format_event_line(event: &Event<'_>, style: FormatStyle) -> String {
  let mut visitor = FieldVisitor::default();
  event.record(&mut visitor);
  match style {
    FormatStyle::Cli => {
      if visitor.message.is_empty() {
        String::new()
      } else {
        format!("{}\n", visitor.message)
      }
    }
    FormatStyle::Structured => {
      let mut line = String::new();
      Iso8601Time.format_time(&mut Writer::new(&mut line)).ok();
      if !line.is_empty() {
        line.push(' ');
      }
      line.push_str(level_label(event.metadata().level()));
      line.push(' ');
      line.push_str(event.metadata().target());
      for (name, value) in visitor.fields {
        let rendered = if matches!(name.as_str(), "bundle_name" | "bundleName") {
          anonymize_bundle_name(&value)
        } else {
          value
        };
        line.push(' ');
        line.push_str(&name);
        line.push('=');
        line.push_str(&rendered);
      }
      if !visitor.message.is_empty() {
        line.push(' ');
        line.push_str(&visitor.message);
      }
      line.push('\n');
      line
    }
  }
}

fn level_label(level: &Level) -> &'static str {
  match *level {
    Level::ERROR => "ERROR",
    Level::WARN => "WARN",
    Level::INFO => "INFO",
    Level::DEBUG => "DEBUG",
    Level::TRACE => "TRACE",
  }
}

fn anonymize_bundle_name(value: &str) -> String {
  let trimmed = value.trim_matches('"');
  if trimmed.is_empty() {
    return "*****".to_string();
  }
  let mut chars = trimmed.chars();
  let first = chars.next().unwrap_or('*');
  let last = chars.next_back().unwrap_or(first);
  if trimmed.chars().count() == 1 {
    format!("{first}***{first}")
  } else {
    format!("{first}***{last}")
  }
}

#[derive(Default)]
struct FieldVisitor {
  message: String,
  fields: Vec<(String, String)>,
}

impl tracing::field::Visit for FieldVisitor {
  fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
    let rendered = format!("{value:?}");
    let rendered = rendered
      .strip_prefix('"')
      .and_then(|value| value.strip_suffix('"'))
      .unwrap_or(&rendered)
      .to_string();
    if field.name() == "message" {
      self.message = rendered;
    } else {
      self.fields.push((field.name().to_string(), rendered));
    }
  }

  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "message" {
      self.message = value.to_string();
    } else {
      self
        .fields
        .push((field.name().to_string(), value.to_string()));
    }
  }

  fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
    self
      .fields
      .push((field.name().to_string(), value.to_string()));
  }

  fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
    self
      .fields
      .push((field.name().to_string(), value.to_string()));
  }

  fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
    self
      .fields
      .push((field.name().to_string(), value.to_string()));
  }
}

struct Iso8601Time;

impl FormatTime for Iso8601Time {
  fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = tod / 3600;
    let minute = (tod % 3600) / 60;
    let second = tod % 60;
    write!(
      w,
      "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
    )
  }
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = (z - era * 146_097) as u64;
  let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = if mp < 10 { mp + 3 } else { mp - 9 };
  let y = if m <= 2 { y + 1 } else { y };
  (y as i32, m as u32, d as u32)
}
