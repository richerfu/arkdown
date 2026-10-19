use std::sync::Arc;

use napi::bindgen_prelude::{FnArgs, Function};
use napi::threadsafe_function::{
  ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::{Result, Status};
use napi_derive::napi;
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{EnvFilter, Layer};

/// A build-scoped diagnostic or stage event. Console presentation belongs to JavaScript.
#[napi(object, object_from_js = false)]
pub struct BuildLog {
  #[napi(ts_type = "'debug' | 'info' | 'warn' | 'error'")]
  pub level: String,
  pub target: String,
  pub message: String,
  #[napi(ts_type = "Record<string, string | number | boolean | null>")]
  pub fields: serde_json::Value,
}

type LogCallback = ThreadsafeFunction<BuildLog, (), FnArgs<(BuildLog,)>, Status, false, false>;

/// Each request owns its subscriber. No process-global filters, callbacks or log files.
pub(crate) struct RuntimeObservability {
  callback: Arc<LogCallback>,
  dispatch: tracing::Dispatch,
}

impl RuntimeObservability {
  pub(crate) fn new(callback: Function<'_, BuildLog, ()>, level: Option<&str>) -> Result<Self> {
    let threshold = match level.unwrap_or("info") {
      "silent" => tracing::metadata::LevelFilter::OFF,
      "error" => tracing::metadata::LevelFilter::ERROR,
      "warn" => tracing::metadata::LevelFilter::WARN,
      "info" => tracing::metadata::LevelFilter::INFO,
      "debug" => tracing::metadata::LevelFilter::DEBUG,
      other => {
        return Err(napi::Error::from_reason(format!(
          "invalid logLevel '{other}'; expected debug, info, warn, error, or silent"
        )))
      }
    };
    let filter = std::env::var("ARKDOWN_LOG")
      .ok()
      .or_else(|| std::env::var("RUST_LOG").ok())
      .filter(|spec| !spec.trim().is_empty())
      .map(EnvFilter::try_new)
      .transpose()
      .map_err(|error| napi::Error::from_reason(format!("invalid build log filter: {error}")))?
      .unwrap_or_else(|| EnvFilter::new("trace"));
    let callback = Arc::new(
      callback
        .build_threadsafe_function::<BuildLog>()
        .build_callback(|ctx: ThreadsafeCallContext<BuildLog>| Ok(FnArgs { data: (ctx.value,) }))?,
    );
    let layer = BuildLogLayer {
      callback: Arc::clone(&callback),
    };
    Ok(Self {
      callback,
      dispatch: tracing::Dispatch::new(
        tracing_subscriber::registry().with(layer.with_filter(filter).with_filter(threshold)),
      ),
    })
  }

  pub(crate) fn dispatch(&self) -> tracing::Dispatch {
    self.dispatch.clone()
  }

  /// A FIFO barrier ensures diagnostics reach JS before the build promise settles.
  pub(crate) async fn flush(&self) -> Result<()> {
    self
      .callback
      .call_async(BuildLog {
        level: "debug".into(),
        target: "arkdown::log_flush".into(),
        message: String::new(),
        fields: serde_json::Value::Null,
      })
      .await
  }
}

struct BuildLogLayer {
  callback: Arc<LogCallback>,
}

impl<S: Subscriber> Layer<S> for BuildLogLayer {
  fn enabled(&self, metadata: &Metadata<'_>, _context: Context<'_, S>) -> bool {
    metadata.target().starts_with("arkdown")
  }

  fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
    let mut visitor = BuildLogVisitor::default();
    event.record(&mut visitor);
    let level = match *event.metadata().level() {
      Level::ERROR => "error",
      Level::WARN => "warn",
      Level::INFO => "info",
      _ => "debug",
    };
    self.callback.call(
      BuildLog {
        level: level.into(),
        target: event.metadata().target().into(),
        message: visitor.message,
        fields: serde_json::Value::Object(visitor.fields),
      },
      ThreadsafeFunctionCallMode::NonBlocking,
    );
  }
}

#[derive(Default)]
struct BuildLogVisitor {
  message: String,
  fields: serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for BuildLogVisitor {
  fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
    self.record_str(field, &format!("{value:?}"));
  }

  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "message" {
      self.message = value.into();
    } else {
      self.fields.insert(field.name().into(), value.into());
    }
  }

  fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
    self.fields.insert(field.name().into(), value.into());
  }

  fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
    self.fields.insert(field.name().into(), value.into());
  }

  fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
    self.fields.insert(field.name().into(), value.into());
  }

  fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
    self.fields.insert(field.name().into(), value.into());
  }
}
