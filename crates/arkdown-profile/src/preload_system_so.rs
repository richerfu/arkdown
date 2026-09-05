use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use arkdown_core::{ModuleJson5, OhosModuleType, PreloadSystemSoTask};
use arkdown_session::dependency_graph::snapshot::{DependencyGraphSnapshot, DependencyNodeType};

/// Plans the entry roots used by hvigor's `needPreloadSoFileSet`.
pub struct PreloadSystemSoPlan<'a> {
    module_root: &'a Path,
    module_json: &'a ModuleJson5,
    dependencies: &'a DependencyGraphSnapshot,
}

impl<'a> PreloadSystemSoPlan<'a> {
    pub fn new(
        module_root: &'a Path,
        module_json: &'a ModuleJson5,
        dependencies: &'a DependencyGraphSnapshot,
    ) -> Self {
        Self {
            module_root,
            module_json,
            dependencies,
        }
    }

    pub fn entries(&self) -> Result<Vec<PathBuf>> {
        let mut entries = BTreeSet::new();
        self.collect_module_entries(self.module_root, self.module_json, &mut entries);

        for dependency in self.dependencies.nodes.values() {
            if dependency.node_type != DependencyNodeType::SourceHar {
                continue;
            }
            let Some(root) = dependency.package_root.as_deref() else {
                continue;
            };
            let Some(profile) = dependency.module_profile.as_ref() else {
                continue;
            };
            let src_main = root.join("src").join("main");
            entries.extend(
                profile
                    .home_ability_src_entries
                    .iter()
                    .map(|entry| arkdown_path::canonicalize_or_owned(&src_main.join(entry))),
            );
        }

        Ok(entries.into_iter().collect())
    }

    fn collect_module_entries(
        &self,
        module_root: &Path,
        module_json: &ModuleJson5,
        entries: &mut BTreeSet<PathBuf>,
    ) {
        let src_main = module_root.join("src").join("main");
        if let Some(src_entry) = module_json.module.src_entry.as_deref() {
            entries.insert(arkdown_path::canonicalize_or_owned(
                &src_main.join(src_entry),
            ));
        }
        for ability in &module_json.module.abilities {
            if Self::is_home_ability(ability) {
                entries.insert(arkdown_path::canonicalize_or_owned(
                    &src_main.join(&ability.src_entry),
                ));
            }
        }
    }

    fn is_home_ability(ability: &arkdown_core::AbilityConfig) -> bool {
        !ability.src_entry.is_empty()
            && ability.skills.iter().any(|skill| {
                skill
                    .entities
                    .iter()
                    .any(|entity| entity == "entity.system.home")
                    && skill.actions.iter().any(|action| {
                        matches!(
                            action.as_str(),
                            "action.system.home" | "ohos.want.action.home"
                        )
                    })
            })
    }
}

/// Writes the SDK-compatible preload file and, for HAP modules, merges it into
/// the compiled startup resource just like hvigor `ProcessCompiledResources`.
pub struct PreloadSystemSoArtifactWriter<'a> {
    preload_json_path: &'a Path,
    compiled_profile_dir: &'a Path,
    processed_module_json_path: &'a Path,
    module_type: &'a OhosModuleType,
    tasks: &'a [PreloadSystemSoTask],
}

impl<'a> PreloadSystemSoArtifactWriter<'a> {
    pub fn new(
        preload_json_path: &'a Path,
        compiled_profile_dir: &'a Path,
        processed_module_json_path: &'a Path,
        module_type: &'a OhosModuleType,
        tasks: &'a [PreloadSystemSoTask],
    ) -> Self {
        Self {
            preload_json_path,
            compiled_profile_dir,
            processed_module_json_path,
            module_type,
            tasks,
        }
    }

    /// Materialize the CompileArkTS sidecar consumed by hvigor's later
    /// `ProcessCompiledResources` task without mutating compiled resources yet.
    pub fn write_preload_artifact(&self) -> Result<()> {
        self.write_preload_json()
    }

    /// Apply hvigor `ProcessCompiledResources` after `GeneratePkgModuleJson`.
    pub fn process_compiled_resources(&self) -> Result<()> {
        if self.tasks.is_empty()
            || !matches!(
                self.module_type,
                OhosModuleType::Entry | OhosModuleType::Feature
            )
        {
            return Ok(());
        }
        self.merge_compiled_startup_profile()?;
        self.ensure_module_startup_reference()
    }

    pub fn effective_path(path: &Path) -> Result<Option<PathBuf>> {
        if !path.is_file() {
            return Ok(None);
        }
        let value = serde_json::from_slice::<serde_json::Value>(&fs_err::read(path)?)?;
        Ok(value
            .get("systemPreloadHintStartupTasks")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tasks| !tasks.is_empty())
            .then(|| path.to_path_buf()))
    }

    fn write_preload_json(&self) -> Result<()> {
        if let Some(parent) = self.preload_json_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let tasks = self
            .tasks
            .iter()
            .map(|task| {
                serde_json::json!({
                  "name": task.name,
                  "srcEntry": task.src_entry,
                  "ohmurl": task.ohm_url,
                })
            })
            .collect::<Vec<_>>();
        fs_err::write(
            self.preload_json_path,
            serde_json::to_vec_pretty(&serde_json::json!({
              "systemPreloadHintStartupTasks": tasks,
            }))?,
        )?;
        Ok(())
    }

    fn merge_compiled_startup_profile(&self) -> Result<()> {
        fs_err::create_dir_all(self.compiled_profile_dir)?;
        let path = self.compiled_profile_dir.join("startup_config.json");
        let mut profile = if path.is_file() {
            serde_json::from_slice::<serde_json::Value>(&fs_err::read(&path)?)?
        } else {
            serde_json::json!({})
        };
        let object = profile
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("compiled startup_config.json must be an object"))?;
        let existing = object
            .entry("systemPreloadHintStartupTasks")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "startup_config.json systemPreloadHintStartupTasks must be an array"
                )
            })?;
        let mut names = existing
            .iter()
            .filter_map(|task| task.get("name").and_then(serde_json::Value::as_str))
            .map(ToOwned::to_owned)
            .collect::<BTreeSet<_>>();
        for task in self.tasks {
            if names.insert(task.name.clone()) {
                existing.push(serde_json::json!({
                  "name": task.name,
                  "srcEntry": task.src_entry,
                  "ohmurl": task.ohm_url,
                }));
            }
        }
        fs_err::write(path, serde_json::to_vec(&profile)?)?;
        Ok(())
    }

    fn ensure_module_startup_reference(&self) -> Result<()> {
        let mut module_json = serde_json::from_slice::<serde_json::Value>(&fs_err::read(
            self.processed_module_json_path,
        )?)?;
        let module = module_json
            .get_mut("module")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| anyhow::anyhow!("processed module.json must contain module object"))?;
        module
            .entry("appStartup")
            .or_insert_with(|| serde_json::Value::String("$profile:startup_config".to_string()));
        fs_err::write(
            self.processed_module_json_path,
            serde_json::to_vec(&module_json)?,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{AbilityConfig, SkillConfig};
    use arkdown_session::dependency_graph::snapshot::{
        DependencyModuleProfileSnapshot, DependencyNodeSnapshot,
    };

    #[test]
    fn home_ability_matches_hvigor_entity_and_action_contract() {
        let ability = AbilityConfig {
            name: "EntryAbility".to_string(),
            src_entry: "ets/EntryAbility.ets".to_string(),
            exported: true,
            skills: vec![SkillConfig {
                actions: vec!["action.system.home".to_string()],
                entities: vec!["entity.system.home".to_string()],
                uris: vec![],
            }],
            icon: None,
            label: None,
            description: None,
            start_window_icon: None,
            start_window_background: None,
        };
        assert!(PreloadSystemSoPlan::is_home_ability(&ability));
    }

    #[test]
    fn dependency_entries_are_consumed_from_the_build_session_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let module_root = temp.path().join("entry");
        let dependency_root = temp.path().join("source-har");
        let module_json = json5::from_str::<ModuleJson5>(
            r#"{ module: { name: "entry", type: "entry", srcEntry: "ets/EntryAbility.ets" } }"#,
        )
        .unwrap();
        let mut dependencies = DependencyGraphSnapshot::default();
        dependencies.nodes.insert(
            "source-har".to_string(),
            DependencyNodeSnapshot {
                alias: "source-har".to_string(),
                package_name: "source-har".to_string(),
                version_spec: None,
                package_root: Some(dependency_root.clone()),
                package: None,
                module_profile: Some(DependencyModuleProfileSnapshot {
                    home_ability_src_entries: vec!["ets/HomeAbility.ets".to_string()],
                    ..Default::default()
                }),
                compile_inputs: Default::default(),
                node_type: DependencyNodeType::SourceHar,
                is_runtime_only: false,
                is_dynamic: false,
                is_transitive: false,
                direct_deps: Vec::new(),
            },
        );

        let entries = PreloadSystemSoPlan::new(&module_root, &module_json, &dependencies)
            .entries()
            .unwrap();

        assert_eq!(
            entries,
            vec![
                arkdown_path::canonicalize_or_owned(
                    &module_root.join("src/main/ets/EntryAbility.ets"),
                ),
                arkdown_path::canonicalize_or_owned(
                    &dependency_root.join("src/main/ets/HomeAbility.ets"),
                ),
            ]
        );
    }

    #[test]
    fn writer_merges_tasks_and_sets_app_startup_reference() {
        let temp = tempfile::tempdir().unwrap();
        let preload = temp
            .path()
            .join("intermediates/preload/default/preload.json");
        let profile = temp
            .path()
            .join("intermediates/res/default/resources/base/profile");
        let module_json = temp.path().join("intermediates/res/default/module.json");
        fs_err::create_dir_all(module_json.parent().unwrap()).unwrap();
        fs_err::write(&module_json, br#"{"module":{"name":"entry"}}"#).unwrap();
        let tasks = vec![PreloadSystemSoTask {
            name: "@ohos.hilog".to_string(),
            src_entry: "@ohos.hilog".to_string(),
            ohm_url: "@ohos:hilog".to_string(),
        }];

        let writer = PreloadSystemSoArtifactWriter::new(
            &preload,
            &profile,
            &module_json,
            &OhosModuleType::Entry,
            &tasks,
        );
        writer.write_preload_artifact().unwrap();
        writer.process_compiled_resources().unwrap();

        let preload_value: serde_json::Value =
            serde_json::from_slice(&fs_err::read(preload).unwrap()).unwrap();
        assert_eq!(
            preload_value["systemPreloadHintStartupTasks"][0]["ohmurl"],
            "@ohos:hilog"
        );
        let module_value: serde_json::Value =
            serde_json::from_slice(&fs_err::read(module_json).unwrap()).unwrap();
        assert_eq!(
            module_value["module"]["appStartup"],
            "$profile:startup_config"
        );
    }
}
