//! Node.js subprocess runner for external JavaScript compile plugins.
//!
//! Bridges `compilePluginPath` JavaScript plugins into the Rolldown plugin
//! pipeline by spawning a persistent Node.js subprocess and communicating
//! via a JSON-RPC protocol over stdin/stdout.
//!
//! # Protocol
//!
//! Every hook call sends one newline-delimited JSON request and reads one
//! newline-delimited JSON response:
//!
//! ```text
//! → {"id":1,"hook":"transform","args":["code","file.js"]}\n
//! ← {"id":1,"result":"...transformed code..."}\n
//! ← {"id":1,"error":"something went wrong"}\n   (on failure)
//! ```
//!
//! # Lifecycle
//!
//! One subprocess per plugin file. The subprocess is started lazily on the
//! first hook call and kept alive for the entire build. It is dropped when
//! the `NodeJsPlugin` is dropped.
//!
//! # Requirements
//!
//! Node.js (>=16) must be on the PATH or set via `NODE_BINARY` env var.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use arkdown_path::ArtifactPathSerializer;
use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
    HookResolveIdReturn, HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage,
    Plugin, PluginContext, SharedLoadPluginContext, SharedTransformPluginContext,
};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Runner script
// ---------------------------------------------------------------------------

/// The Node.js runner script injected via `--eval`. It:
/// 1. Loads the plugin file given as `argv[2]`
/// 2. Reads newline-delimited JSON requests from stdin
/// 3. Calls the requested hook function
/// 4. Writes newline-delimited JSON responses to stdout
const RUNNER_SCRIPT: &str = r#"
'use strict';
const readline = require('readline');
const path = require('path');

const pluginPath = path.resolve(process.argv[2]);
let plugin;
try {
  const mod = require(pluginPath);
  plugin = (mod && mod.__esModule && mod.default) ? mod.default : (mod.default || mod);
} catch (e) {
  process.stderr.write('PLUGIN_LOAD_ERROR: ' + e.message + '\n');
  process.exit(1);
}

const rl = readline.createInterface({ input: process.stdin, terminal: false });

rl.on('line', async (line) => {
  let req;
  try { req = JSON.parse(line); } catch (_) { return; }
  const { id, hook, args } = req;
  try {
    const fn_ = plugin[hook];
    if (typeof fn_ !== 'function') {
      process.stdout.write(JSON.stringify({ id, result: null }) + '\n');
      return;
    }
    const result = await fn_.apply(plugin, args || []);
    process.stdout.write(JSON.stringify({ id, result: result !== undefined ? result : null }) + '\n');
  } catch (e) {
    process.stdout.write(JSON.stringify({ id, error: e && e.message ? e.message : String(e) }) + '\n');
  }
});

rl.on('close', () => process.exit(0));
"#;

// ---------------------------------------------------------------------------
// NodeJsConn — owns the subprocess handle
// ---------------------------------------------------------------------------

struct NodePluginPathText<'a> {
    path: &'a Path,
}

impl<'a> NodePluginPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(&self) -> String {
        ArtifactPathSerializer::path_to_slash_string(self.path)
            .unwrap_or_else(|_| self.path.display().to_string())
    }
}

struct NodeJsConn {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl NodeJsConn {
    async fn spawn(plugin_path: &Path) -> Result<Self> {
        let node_bin = std::env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string());

        let mut child = Command::new(&node_bin)
            .arg("--eval")
            .arg(RUNNER_SCRIPT)
            .arg("--") // argv[1] = "--"
            .arg(plugin_path) // argv[2] = plugin path
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to spawn Node.js for compile plugin {}: {} \
                 (is Node.js installed? set NODE_BINARY env var to override)",
                    plugin_path.display(),
                    e
                )
            })?;

        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

        Ok(Self {
            _child: child,
            stdin,
            stdout,
            next_id: 1,
        })
    }

    async fn call(&mut self, hook: &str, args: Vec<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let request = serde_json::json!({ "id": id, "hook": hook, "args": args });
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');

        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("write to Node.js plugin stdin: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("flush Node.js plugin stdin: {e}"))?;

        // Read lines until we find the response with matching id.
        let mut response_line = String::new();
        loop {
            response_line.clear();
            let n = self
                .stdout
                .read_line(&mut response_line)
                .await
                .map_err(|e| anyhow::anyhow!("read from Node.js plugin stdout: {e}"))?;
            if n == 0 {
                anyhow::bail!("Node.js plugin process closed stdout unexpectedly");
            }
            let resp: Value = serde_json::from_str(response_line.trim())
                .map_err(|e| anyhow::anyhow!("invalid JSON from Node.js plugin: {e}"))?;
            if resp.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
                    anyhow::bail!("Node.js plugin hook '{}' error: {}", hook, err);
                }
                return Ok(resp.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NodeJsPlugin
// ---------------------------------------------------------------------------

/// A Rolldown `Plugin` implementation that calls hooks in an external
/// JavaScript plugin file via a persistent Node.js subprocess.
pub struct NodeJsPlugin {
    plugin_name: String,
    plugin_path: PathBuf,
    conn: Arc<Mutex<Option<NodeJsConn>>>,
    /// Which hooks the plugin registers (detected at construction time via
    /// a lightweight `require` + property inspection).
    usage: HookUsage,
}

impl std::fmt::Debug for NodeJsPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeJsPlugin")
            .field("name", &self.plugin_name)
            .field("path", &self.plugin_path)
            .finish_non_exhaustive()
    }
}

impl NodeJsPlugin {
    /// Probe a plugin file to detect which hooks it exports and extract its
    /// name.  Returns `None` (with a warning) if Node.js is not available or
    /// the file cannot be loaded.
    pub async fn probe(plugin_path: &Path) -> Option<Arc<Self>> {
        match Self::probe_inner(plugin_path).await {
            Ok(plugin) => Some(Arc::new(plugin)),
            Err(e) => {
                tracing::warn!(
                    path = %plugin_path.display(),
                    error = %e,
                    "failed to probe Node.js compile plugin — skipping"
                );
                None
            }
        }
    }

    async fn probe_inner(plugin_path: &Path) -> Result<Self> {
        let node_bin = std::env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string());

        // Run a quick one-shot Node.js process to extract the plugin's
        // name and exported hook names.
        let probe_script = format!(
            r#"
'use strict';
const path = require('path');
try {{
  const mod = require(path.resolve({}));
  const p = (mod && mod.__esModule && mod.default) ? mod.default : (mod.default || mod);
  const hooks = ['resolveId','load','transform','buildStart','buildEnd',
                 'moduleParsed','beforeBuildEnd','shouldInvalidCache'];
  const present = hooks.filter(h => typeof p[h] === 'function');
  process.stdout.write(`${{JSON.stringify({{ name: p.name || 'unknown', hooks: present }})}}\n`);
}} catch (e) {{
  process.stderr.write(`${{e.message}}\n`);
  process.exit(1);
}}
"#,
            serde_json::to_string(&NodePluginPathText::new(&plugin_path).string()).unwrap()
        );

        let output = Command::new(&node_bin)
            .arg("--eval")
            .arg(&probe_script)
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Node.js not available ({}): {}", node_bin, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("plugin probe failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let info: Value = serde_json::from_str(stdout.trim())
            .map_err(|e| anyhow::anyhow!("probe output parse error: {e}"))?;

        let plugin_name = info["name"].as_str().unwrap_or("unknown").to_string();
        let hook_names: Vec<&str> = info["hooks"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let mut usage = HookUsage::empty();
        for hook in &hook_names {
            match *hook {
                "resolveId" => usage |= HookUsage::ResolveId,
                "load" => usage |= HookUsage::Load,
                "transform" => usage |= HookUsage::Transform,
                "buildStart" => usage |= HookUsage::BuildStart,
                "buildEnd" => usage |= HookUsage::BuildEnd,
                "moduleParsed" => usage |= HookUsage::ModuleParsed,
                _ => {}
            }
        }

        tracing::info!(
            plugin = %plugin_name,
            path = %plugin_path.display(),
            hooks = ?hook_names,
            "loaded Node.js compile plugin"
        );

        Ok(Self {
            plugin_name,
            plugin_path: plugin_path.to_owned(),
            conn: Arc::new(Mutex::new(None)),
            usage,
        })
    }

    /// Get or lazily start the Node.js subprocess.
    async fn conn(&self) -> Result<tokio::sync::MutexGuard<'_, Option<NodeJsConn>>> {
        let mut guard = self.conn.lock().await;
        if guard.is_none() {
            let conn = NodeJsConn::spawn(&self.plugin_path).await?;
            *guard = Some(conn);
        }
        Ok(guard)
    }

    async fn call_hook(&self, hook: &str, args: Vec<Value>) -> Result<Value> {
        let mut guard = self.conn().await?;
        let conn = guard.as_mut().expect("conn initialized above");
        conn.call(hook, args).await
    }
}

impl Plugin for NodeJsPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Owned(self.plugin_name.clone())
    }

    fn register_hook_usage(&self) -> HookUsage {
        self.usage
    }

    fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
        let specifier = args.specifier.to_string();
        let importer = args.importer.map(|s| s.to_string());
        let plugin = self.clone_arc();
        async move {
            let result = plugin
                .call_hook(
                    "resolveId",
                    vec![
                        Value::String(specifier),
                        importer.map(Value::String).unwrap_or(Value::Null),
                    ],
                )
                .await?;

            match result {
                Value::String(id) => Ok(Some(HookResolveIdOutput {
                    id: id.into(),
                    ..Default::default()
                })),
                Value::Null => Ok(None),
                _ => Ok(None),
            }
        }
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let id = args.id.to_string();
        let plugin = self.clone_arc();
        async move {
            let result = plugin.call_hook("load", vec![Value::String(id)]).await?;

            match result {
                Value::String(code) => Ok(Some(HookLoadOutput {
                    code: code.into(),
                    ..Default::default()
                })),
                Value::Null => Ok(None),
                _ => Ok(None),
            }
        }
    }

    fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
        let code = args.code.to_string();
        let id = args.id.to_string();
        let plugin = self.clone_arc();
        async move {
            let result = plugin
                .call_hook("transform", vec![Value::String(code), Value::String(id)])
                .await?;

            match result {
                Value::String(new_code) => Ok(Some(HookTransformOutput {
                    code: Some(new_code),
                    ..Default::default()
                })),
                Value::Object(obj) => {
                    let new_code = obj.get("code").and_then(|v| v.as_str()).map(String::from);
                    Ok(new_code.map(|code| HookTransformOutput {
                        code: Some(code),
                        ..Default::default()
                    }))
                }
                Value::Null => Ok(None),
                _ => Ok(None),
            }
        }
    }
}

impl NodeJsPlugin {
    fn clone_arc(self: &Self) -> Arc<Self> {
        // SAFETY: `self` is always behind an Arc<Self> (created via probe()).
        // We reconstruct the Arc from self's address using clone of the inner Arc fields.
        Arc::new(Self {
            plugin_name: self.plugin_name.clone(),
            plugin_path: self.plugin_path.clone(),
            conn: Arc::clone(&self.conn),
            usage: self.usage,
        })
    }
}

// ---------------------------------------------------------------------------
// Loader for PluginFactory integration
// ---------------------------------------------------------------------------

/// Loads JavaScript compile plugins through a Node.js subprocess.
pub struct NodeJsPluginLoader;

impl NodeJsPluginLoader {
    /// Load a JavaScript compile plugin via Node.js subprocess.
    ///
    /// Returns `None` when Node.js is not available or the plugin file fails to
    /// load.
    pub async fn load(
        path: &Path,
        order: super::external::PluginOrder,
    ) -> Option<(Arc<NodeJsPlugin>, super::external::PluginOrder)> {
        NodeJsPlugin::probe(path)
            .await
            .map(|plugin| (plugin, order))
    }
}
