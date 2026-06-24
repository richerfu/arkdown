//! Filename uniqueness warning plugin.
//!
//! Mirrors hvigor's `checkDuplicatedFileNamesPlugin`: collect loaded module
//! ids, warn when the same id without extension is seen more than once, and
//! never fail the bundle on this diagnostic.
//!
//! Previously the standalone `arkdown-plugin-duplicate-check` crate.
//!
//! # Hook sequence
//!
//! * `build_start` — clears collected ids so watch-mode rebuilds start from a
//!   clean slate.
//! * `load` — records each module id using hvigor's `<id without extension>`
//!   key.
//! * `build_end` — emits one warning if duplicate ids were found and clears
//!   state.

use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::path::Path;
use std::sync::{Arc, Mutex};

use arkdown_path::{OhmUrl, SourceFileName};
use rolldown_common::LogWithoutPlugin;
use rolldown_plugin::{
    HookBuildEndArgs, HookBuildStartArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookUsage,
    Plugin, PluginContext, SharedLoadPluginContext,
};

/// Plugin that mirrors hvigor's duplicate filename warning behavior.
pub struct DuplicateCheckPlugin {
    state: Arc<Mutex<DuplicateState>>,
}

#[derive(Default)]
struct DuplicateState {
    /// Map from full module id without extension → first module id.
    file_names: FxHashMap<String, String>,
    /// Full module ids involved in duplicate keys.
    duplicated_file_names: FxHashSet<String>,
}

impl DuplicateState {
    fn record_module_id(&mut self, id: String, key: String) {
        if let Some(existing) = self.file_names.get(&key).cloned() {
            self.duplicated_file_names.insert(existing);
            self.duplicated_file_names.insert(id);
        } else {
            self.file_names.insert(key, id);
        }
    }

    fn sorted_duplicate_file_names(&self) -> Vec<String> {
        let mut files = self
            .duplicated_file_names
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    fn clear(&mut self) {
        self.file_names.clear();
        self.duplicated_file_names.clear();
    }
}

impl DuplicateCheckPlugin {
    pub fn new() -> Self {
        DuplicateCheckPlugin {
            state: Arc::new(Mutex::new(DuplicateState::default())),
        }
    }
}

impl Default for DuplicateCheckPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DuplicateCheckPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuplicateCheckPlugin")
            .finish_non_exhaustive()
    }
}

impl Plugin for DuplicateCheckPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown:duplicate-check")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::BuildStart | HookUsage::Load | HookUsage::BuildEnd
    }

    /// Clear the accumulated stem map at the start of each build so that
    /// watch-mode incremental rebuilds do not carry over stale entries.
    fn build_start(
        &self,
        _ctx: &PluginContext,
        _args: &HookBuildStartArgs<'_>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let state = Arc::clone(&self.state);
        async move {
            let mut state = state.lock().map_err(|e| {
                anyhow::anyhow!("duplicate-check: failed to lock state in build_start: {e}")
            })?;
            state.clear();
            Ok(())
        }
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let id = args.id.to_owned();
        let state = Arc::clone(&self.state);

        async move {
            // Skip declaration files — they are type-only and produce no records.
            if DuplicateModuleId::new(&id).is_declaration() {
                return Ok(None);
            }

            // Skip OHM URL external references — not local source files.
            if OhmUrl::parse(&id)
                .ok()
                .is_some_and(|url| url.is_cross_bundle_reference())
            {
                return Ok(None);
            }

            let module_id = DuplicateModuleId::new(&id);
            let key = match module_id.extensionless_key() {
                Some(key) => key,
                None => return Ok(None),
            };

            let mut state = state.lock().map_err(|e| {
                anyhow::anyhow!("duplicate-check: failed to lock state in load: {e}")
            })?;

            state.record_module_id(id, key);

            Ok(None)
        }
    }

    fn build_end(
        &self,
        ctx: &PluginContext,
        _args: Option<&HookBuildEndArgs<'_>>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let ctx = ctx.clone();
        let state = Arc::clone(&self.state);

        async move {
            let mut state = state.lock().map_err(|e| {
                anyhow::anyhow!("duplicate-check: failed to lock state in build_end: {e}")
            })?;
            if !state.duplicated_file_names.is_empty() {
                let files = state.sorted_duplicate_file_names();
                ctx.warn(LogWithoutPlugin {
                    message: format!(
                        "Duplicate file names detected, which may cause compilation errors. \
                         Please check the following files:\n{}",
                        files.join("\n")
                    ),
                    code: Some("DUPLICATE_FILENAMES".to_string()),
                    ids: Some(files),
                    ..Default::default()
                });
            }
            state.clear();
            Ok(())
        }
    }
}

struct DuplicateModuleId<'a> {
    raw: &'a str,
}

impl<'a> DuplicateModuleId<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_declaration(&self) -> bool {
        SourceFileName::new(Path::new(self.raw)).is_declaration()
    }

    fn extensionless_key(&self) -> Option<String> {
        let mut path = Path::new(self.raw).to_path_buf();
        path.extension()?;
        path.set_extension("");
        path.to_str().map(ToOwned::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::{DuplicateModuleId, DuplicateState};

    #[test]
    fn keeps_directory_in_extensionless_key_like_hvigor() {
        assert_eq!(
            DuplicateModuleId::new("/project/src/a/Index.ets").extensionless_key(),
            Some("/project/src/a/Index".to_owned())
        );
        assert_eq!(
            DuplicateModuleId::new("/project/src/b/Index.ets").extensionless_key(),
            Some("/project/src/b/Index".to_owned())
        );
    }

    #[test]
    fn different_directories_with_same_file_name_are_not_duplicates() {
        let mut state = DuplicateState::default();
        for id in ["/project/src/a/Index.ets", "/project/src/b/Index.ets"] {
            state.record_module_id(
                id.to_owned(),
                DuplicateModuleId::new(id).extensionless_key().unwrap(),
            );
        }

        assert!(state.sorted_duplicate_file_names().is_empty());
    }

    #[test]
    fn same_path_with_different_extensions_is_duplicate_like_hvigor() {
        let mut state = DuplicateState::default();
        for id in ["/project/src/a/Index.ets", "/project/src/a/Index.ts"] {
            state.record_module_id(
                id.to_owned(),
                DuplicateModuleId::new(id).extensionless_key().unwrap(),
            );
        }

        assert_eq!(
            state.sorted_duplicate_file_names(),
            vec![
                "/project/src/a/Index.ets".to_owned(),
                "/project/src/a/Index.ts".to_owned()
            ]
        );
    }
}
