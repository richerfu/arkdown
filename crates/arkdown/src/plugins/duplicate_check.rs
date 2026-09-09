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

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use rolldown_common::LogWithoutPlugin;
use rolldown_plugin::{
    HookBuildEndArgs, HookBuildStartArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookUsage,
    Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};

/// Plugin that mirrors hvigor's duplicate filename warning behavior.
pub struct DuplicateCheckPlugin {
    state: Arc<DuplicateState>,
}

#[derive(Default)]
struct DuplicateState {
    /// Map from full module id without extension → first module id.
    file_names: FxDashMap<String, String>,
    /// Full module ids involved in duplicate keys.
    duplicated_file_names: FxDashSet<String>,
}

impl DuplicateState {
    fn record_module_id(&self, id: String, key: String) {
        let mut existing = None;
        self.file_names
            .entry(key)
            .and_modify(|value| existing = Some(value.clone()))
            .or_insert_with(|| id.clone());
        if let Some(existing) = existing {
            self.duplicated_file_names.insert(existing);
            self.duplicated_file_names.insert(id);
        }
    }

    fn sorted_duplicate_file_names(&self) -> Vec<String> {
        let mut files = self
            .duplicated_file_names
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    fn clear(&self) {
        self.file_names.clear();
        self.duplicated_file_names.clear();
    }
}

impl DuplicateCheckPlugin {
    pub fn new() -> Self {
        DuplicateCheckPlugin {
            state: Arc::new(DuplicateState::default()),
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
            let module_id = DuplicateModuleId::new(&id);
            let key = module_id.extensionless_key();
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

    /// `check-duplicate-filenames-plugin.js::load` applies Node
    /// `path.extname(id)` and slices that suffix from every loaded module ID.
    /// An extensionless ID therefore uses the empty key, exactly like
    /// `id.slice(0, -0)` in JavaScript.
    fn extensionless_key(&self) -> String {
        let mut path = Path::new(self.raw).to_path_buf();
        if path.extension().is_none() {
            return String::new();
        }
        path.set_extension("");
        path.to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{DuplicateModuleId, DuplicateState};

    #[test]
    fn keeps_directory_in_extensionless_key_like_hvigor() {
        assert_eq!(
            DuplicateModuleId::new("/project/src/a/Index.ets").extensionless_key(),
            "/project/src/a/Index".to_owned()
        );
        assert_eq!(
            DuplicateModuleId::new("/project/src/b/Index.ets").extensionless_key(),
            "/project/src/b/Index".to_owned()
        );
    }

    #[test]
    fn different_directories_with_same_file_name_are_not_duplicates() {
        let state = DuplicateState::default();
        for id in ["/project/src/a/Index.ets", "/project/src/b/Index.ets"] {
            state.record_module_id(
                id.to_owned(),
                DuplicateModuleId::new(id).extensionless_key(),
            );
        }

        assert!(state.sorted_duplicate_file_names().is_empty());
    }

    #[test]
    fn same_path_with_different_extensions_is_duplicate_like_hvigor() {
        let state = DuplicateState::default();
        for id in ["/project/src/a/Index.ets", "/project/src/a/Index.ts"] {
            state.record_module_id(
                id.to_owned(),
                DuplicateModuleId::new(id).extensionless_key(),
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

    #[test]
    fn extensionless_loaded_ids_share_the_empty_key_like_node_path_extname() {
        let state = DuplicateState::default();
        for id in ["\0virtual-a", "\0virtual-b"] {
            state.record_module_id(
                id.to_owned(),
                DuplicateModuleId::new(id).extensionless_key(),
            );
        }

        assert_eq!(
            state.sorted_duplicate_file_names(),
            vec!["\0virtual-a".to_owned(), "\0virtual-b".to_owned()]
        );
    }
}
