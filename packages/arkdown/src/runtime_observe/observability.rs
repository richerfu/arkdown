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

use super::persist::BuildLogWriter;

static GLOBAL: OnceLock<Arc<RuntimeObservability>> = OnceLock::new();
static SUBSCRIBER_READY: OnceLock<()> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

const JS_OFF: &str = "arkdown_napi::hvigor=off,arkdown_napi::metric=off";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsoleOwner {
  Log4js,
  Tracing,
}

#[derive(Debug, Clone)]
pub(crate) struct ObservabilityInstallPlan {
  pub console_owner: ConsoleOwner,
  pub cache_dir: PathBuf,
  pub build_id: Arc<str>,
}

struct ObservabilityState {
  active: bool,
  build_id: Arc<str>,
}

pub(crate) struct RuntimeObservability {
  state: RwLock<ObservabilityState>,
  stderr_filter: Arc<RwLock<ObservabilityFilterState>>,
  file_filter: Arc<RwLock<ObservabilityFilterState>>,
  file_writer: Arc<BuildLogWriter>,
}

impl RuntimeObservability {
  pub(crate) fn install_build_safety_net(config: &crate::BuildConfig) -> anyhow::Result<Arc<Self>> {
    if let Some(existing) = Self::global().filter(|observability| observability.has_active_plan()) {
      return Ok(existing);
    }
    let project = config
      .project
      .as_deref()
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from("."));
    Self::install(ObservabilityInstallPlan {
      console_owner: Self::console_owner_from_env(),
      cache_dir: project.join(".hvigor"),
      build_id: Arc::from("napi-build"),
    })
  }

  pub(crate) fn global() -> Option<Arc<Self>> {
    GLOBAL.get().cloned()
  }

  pub(crate) fn build_id(&self) -> Arc<str> {
    self
      .state
      .read()
      .map(|state| Arc::clone(&state.build_id))
      .unwrap_or_else(|_| Arc::from(""))
  }

  pub(crate) fn has_active_plan(&self) -> bool {
    self.state.read().map(|state| state.active).unwrap_or(false)
  }

  pub(crate) fn reopen_file(&self, cache_dir: &Path) -> anyhow::Result<()> {
    if cache_dir.as_os_str().is_empty() {
      return Ok(());
    }
    self.file_writer.reopen(cache_dir)?;
    Ok(())
  }

  fn install(plan: ObservabilityInstallPlan) -> anyhow::Result<Arc<Self>> {
    let _install_guard = INSTALL_LOCK
      .lock()
      .map_err(|_| anyhow::anyhow!("observability install lock was poisoned"))?;
    let (stderr_spec, file_spec) = Self::filter_specs(&plan);
    if let Some(existing) = GLOBAL.get() {
      existing.apply_plan(&plan, &stderr_spec, &file_spec)?;
      return Ok(Arc::clone(existing));
    }

    let file_writer = BuildLogWriter::new();
    if !plan.cache_dir.as_os_str().is_empty() {
      file_writer.reopen(&plan.cache_dir)?;
    }

    let installed = Arc::new(Self {
      state: RwLock::new(ObservabilityState {
        active: true,
        build_id: Arc::clone(&plan.build_id),
      }),
      stderr_filter: Arc::new(RwLock::new(ObservabilityFilterState::parse(&stderr_spec)?)),
      file_filter: Arc::new(RwLock::new(ObservabilityFilterState::parse(&file_spec)?)),
      file_writer: Arc::clone(&file_writer),
    });

    if SUBSCRIBER_READY.get().is_none() {
      let layer = ObservabilityLayer {
        stderr_filter: Arc::clone(&installed.stderr_filter),
        file_filter: Arc::clone(&installed.file_filter),
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

  fn apply_plan(
    &self,
    plan: &ObservabilityInstallPlan,
    stderr_spec: &str,
    file_spec: &str,
  ) -> anyhow::Result<()> {
    {
      let mut state = self
        .state
        .write()
        .map_err(|_| anyhow::anyhow!("observability state lock was poisoned"))?;
      state.active = true;
      state.build_id = Arc::clone(&plan.build_id);
    }
    self.swap_filter(&self.stderr_filter, stderr_spec)?;
    self.swap_filter(&self.file_filter, file_spec)?;
    if !plan.cache_dir.as_os_str().is_empty() {
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

  fn filter_specs(plan: &ObservabilityInstallPlan) -> (String, String) {
    Self::service_filter_specs(plan)
  }

  fn service_filter_specs(plan: &ObservabilityInstallPlan) -> (String, String) {
    let env_spec = Self::env_log_spec();
    let console_tracing = plan.console_owner == ConsoleOwner::Tracing;
    match (console_tracing, env_spec.as_deref()) {
      (_, Some("off")) => ("off".to_string(), "off".to_string()),
      (false, None) => (
        format!("arkdown=warn,arkdown_napi=warn,{JS_OFF}"),
        "arkdown=info,arkdown_napi=info".to_string(),
      ),
      (false, Some(spec)) => (Self::append_js_off(spec), spec.to_string()),
      (true, None) => {
        let spec = "arkdown=info,arkdown_napi=info".to_string();
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
}

impl ObservabilityFilterState {
  fn parse(spec: &str) -> anyhow::Result<Self> {
    Ok(Self {
      env: EnvFilter::try_new(spec)
        .map_err(|error| anyhow::anyhow!("invalid observability filter {spec:?}: {error}"))?,
    })
  }
}

struct ObservabilityLayer {
  stderr_filter: Arc<RwLock<ObservabilityFilterState>>,
  file_filter: Arc<RwLock<ObservabilityFilterState>>,
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
      let line = format_event_line(event);
      let _ = std::io::Write::write_all(&mut std::io::stderr(), line.as_bytes());
    }
    if self.file_enabled(event.metadata(), ctx) {
      let line = format_event_line(event);
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

fn format_event_line(event: &Event<'_>) -> String {
  let mut visitor = FieldVisitor::default();
  event.record(&mut visitor);
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
