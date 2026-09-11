//! InsightIntent profile/cache materialization.
//!
//! This is the Rust owner of the source compiler's
//! `ParseIntent.writeUserIntentJsonFile` build-end behavior. OXC/Rolldown
//! provide immutable per-source facts; this processor owns all filesystem
//! reads and writes outside transform hooks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Default)]
pub struct InsightIntentModuleInput {
    pub source_file_path: PathBuf,
    pub decorator_file: String,
    pub intents: Vec<InsightIntentInput>,
    pub entities: Vec<InsightIntentEntityInput>,
}

#[derive(Debug, Clone, Default)]
pub struct InsightIntentInput {
    pub data: Map<String, Value>,
    pub entity_type_names: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InsightIntentEntityInput {
    pub class_name: String,
    pub data: Map<String, Value>,
    pub parent_class_name: Option<String>,
}

pub struct InsightIntentProfileInput<'a> {
    pub profile_path: &'a Path,
    pub cache_path: &'a Path,
    pub module_json_path: &'a Path,
    pub bundle_name: &'a str,
    pub module_name: &'a str,
    pub modules: Vec<InsightIntentModuleInput>,
    pub dependency_profiles: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InsightIntentProfileOutput {
    pub extract_intent_count: usize,
    pub entity_count: usize,
    pub wrote_profile: bool,
}

struct InsightIntentCacheState {
    intents: Vec<Value>,
    entity_owner_map: BTreeMap<String, Vec<String>>,
    entity_map: BTreeMap<String, Value>,
    entity_extends_map: BTreeMap<String, String>,
    heritage_class_set: BTreeSet<String>,
    entity_heritage_class_set: BTreeSet<String>,
    source_file_path_map: BTreeMap<String, String>,
}

impl Default for InsightIntentCacheState {
    fn default() -> Self {
        Self {
            intents: Vec::new(),
            entity_owner_map: BTreeMap::new(),
            entity_map: BTreeMap::new(),
            entity_extends_map: BTreeMap::new(),
            heritage_class_set: [
                "IntentEntity_sdk",
                "InsightIntent.AppIntentEntity_sdk",
                "insightIntent.AppIntentEntity_sdk",
                "InsightIntentEntryExecutor_sdk",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            entity_heritage_class_set: BTreeSet::new(),
            source_file_path_map: BTreeMap::new(),
        }
    }
}

#[derive(Default)]
pub struct InsightIntentProfileProcessor;

impl InsightIntentProfileProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process(
        &self,
        input: InsightIntentProfileInput<'_>,
    ) -> Result<InsightIntentProfileOutput> {
        let dependency_profiles = self.read_dependency_profiles(&input.dependency_profiles)?;
        // `rollup-plugin-ets-typescript.ts::afterBuildEnd` calls
        // `writeUserIntentJsonFile` only when the current compile extracted an
        // intent, incremental cache state exists, or a bytecode HAR contributed
        // non-empty intent data. Preserve that source-owned gate so ordinary
        // cold builds do not read and rewrite profile/module metadata.
        if !input.cache_path.is_file()
            && input.modules.iter().all(|module| module.intents.is_empty())
            && dependency_profiles.is_empty()
        {
            return Ok(InsightIntentProfileOutput::default());
        }
        let mut state = self.current_state(&input.modules)?;
        self.merge_incremental_cache(input.cache_path, &input.modules, &mut state)?;
        self.retain_entity_owners(&mut state);
        self.rebuild_heritage(&mut state);
        self.verify_inheritance(&state)?;
        self.attach_entities(&mut state);
        state
            .intents
            .sort_by(|left, right| self.decorator_file(left).cmp(&self.decorator_file(right)));

        let existing_profile = self.read_object(input.profile_path)?;
        let preserve_extract_field = existing_profile.is_some();
        let mut profile = existing_profile.unwrap_or_default();
        if !state.intents.is_empty() || preserve_extract_field {
            profile.insert(
                "extractInsightIntents".to_string(),
                Value::Array(state.intents.clone()),
            );
        } else {
            profile.remove("extractInsightIntents");
        }
        self.merge_dependency_profiles(
            &dependency_profiles,
            input.bundle_name,
            input.module_name,
            preserve_extract_field,
            &mut profile,
        )?;
        self.validate_unique_intent_names(&profile)?;

        let has_profile_data = !profile.is_empty();
        if has_profile_data {
            self.write_pretty_json(input.profile_path, &Value::Object(profile))?;
            self.write_cache(input.cache_path, &state)?;
        } else {
            self.remove_if_exists(input.profile_path)?;
            self.remove_if_exists(input.cache_path)?;
        }
        self.update_module_json(input.module_json_path, has_profile_data)?;

        Ok(InsightIntentProfileOutput {
            extract_intent_count: state.intents.len(),
            entity_count: state.entity_map.len(),
            wrote_profile: has_profile_data,
        })
    }

    fn current_state(
        &self,
        modules: &[InsightIntentModuleInput],
    ) -> Result<InsightIntentCacheState> {
        let mut state = InsightIntentCacheState::default();
        for module in modules {
            if !module.intents.is_empty() {
                state.source_file_path_map.insert(
                    module.decorator_file.clone(),
                    module.source_file_path.to_string_lossy().into_owned(),
                );
            }
            for intent in &module.intents {
                if let Some(intent_name) = intent.data.get("intentName").and_then(Value::as_str) {
                    if !intent.entity_type_names.is_empty() {
                        state
                            .entity_owner_map
                            .entry(intent_name.to_string())
                            .or_default()
                            .extend(intent.entity_type_names.iter().cloned());
                    }
                }
                state.intents.push(Value::Object(intent.data.clone()));
            }
            for entity in &module.entities {
                if state
                    .entity_map
                    .insert(
                        entity.class_name.clone(),
                        Value::Object(entity.data.clone()),
                    )
                    .is_some()
                {
                    anyhow::bail!(
                        "[10110020] Multiple @InsightIntentEntity decorators applied to class {}",
                        entity.class_name
                    );
                }
                if let Some(parent) = entity.parent_class_name.as_ref() {
                    if !Self::is_sdk_entity_parent(parent) {
                        state
                            .entity_extends_map
                            .insert(entity.class_name.clone(), parent.clone());
                    }
                }
            }
        }
        for names in state.entity_owner_map.values_mut() {
            names.sort();
            names.dedup();
        }
        Ok(state)
    }

    fn merge_incremental_cache(
        &self,
        cache_path: &Path,
        modules: &[InsightIntentModuleInput],
        state: &mut InsightIntentCacheState,
    ) -> Result<()> {
        let Some(cache) = self.read_object(cache_path)? else {
            return Ok(());
        };
        let current_decorator_files = modules
            .iter()
            .map(|module| module.decorator_file.as_str())
            .collect::<std::collections::HashSet<_>>();
        let cached_paths = cache
            .get("sourceFilePathMap")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let retained_decorator = |decorator_file: &str| {
            if current_decorator_files.contains(decorator_file) {
                return false;
            }
            cached_paths
                .get(decorator_file)
                .and_then(Value::as_str)
                .is_some_and(|path| Path::new(path).is_file())
        };

        if let Some(intents) = cache.get("extractInsightIntents").and_then(Value::as_array) {
            state.intents.extend(
                intents
                    .iter()
                    .filter(|intent| self.decorator_file(intent).is_some_and(retained_decorator))
                    .cloned(),
            );
        }
        if let Some(entities) = cache.get("entityMap").and_then(Value::as_object) {
            for (class_name, entity) in entities {
                if self.decorator_file(entity).is_some_and(retained_decorator) {
                    state
                        .entity_map
                        .entry(class_name.clone())
                        .or_insert_with(|| entity.clone());
                }
            }
        }
        if let Some(owners) = cache.get("entityOwnerMap").and_then(Value::as_object) {
            for (intent_name, names) in owners {
                if state.entity_owner_map.contains_key(intent_name) {
                    continue;
                }
                let names = names
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if !names.is_empty() {
                    state.entity_owner_map.insert(intent_name.clone(), names);
                }
            }
        }
        if let Some(extends) = cache.get("entityExtendsMap").and_then(Value::as_object) {
            for (class_name, parent) in extends {
                if let Some(parent) = parent.as_str() {
                    state
                        .entity_extends_map
                        .entry(class_name.clone())
                        .or_insert_with(|| parent.to_string());
                }
            }
        }
        for (decorator_file, path) in &cached_paths {
            if retained_decorator(decorator_file) {
                if let Some(path) = path.as_str() {
                    state
                        .source_file_path_map
                        .entry(decorator_file.clone())
                        .or_insert_with(|| path.to_string());
                }
            }
        }
        Ok(())
    }

    fn rebuild_heritage(&self, state: &mut InsightIntentCacheState) {
        state
            .heritage_class_set
            .retain(|entry| entry.ends_with("_sdk"));
        state.entity_heritage_class_set.clear();
        for (class_name, entity) in &state.entity_map {
            if let Some(decorator_file) = self.decorator_file(entity) {
                state
                    .heritage_class_set
                    .insert(format!("{class_name}_{decorator_file}"));
            }
            let parent = entity
                .get("parentClassName")
                .and_then(Value::as_str)
                .or_else(|| state.entity_extends_map.get(class_name).map(String::as_str));
            let Some(parent) = parent else {
                continue;
            };
            if Self::is_sdk_entity_parent(parent) {
                state
                    .entity_heritage_class_set
                    .insert(format!("{parent}_sdk"));
            } else if let Some(parent_file) = state
                .entity_map
                .get(parent)
                .and_then(|entity| self.decorator_file(entity))
            {
                state
                    .entity_heritage_class_set
                    .insert(format!("{parent}_{parent_file}"));
            } else {
                state
                    .entity_heritage_class_set
                    .insert(format!("{parent}_unresolved"));
            }
        }
    }

    fn retain_entity_owners(&self, state: &mut InsightIntentCacheState) {
        for names in state.entity_owner_map.values_mut() {
            names.retain(|name| state.entity_map.contains_key(name));
            names.sort();
            names.dedup();
        }
        state.entity_owner_map.retain(|_, names| !names.is_empty());
    }

    fn verify_inheritance(&self, state: &InsightIntentCacheState) -> Result<()> {
        if let Some(missing) = state
            .entity_heritage_class_set
            .iter()
            .find(|entry| !state.heritage_class_set.contains(*entry))
        {
            anyhow::bail!(
                "[10110021] Classes decorated with @InsightIntentEntity must implement InsightIntent.IntentEntity: unresolved inheritance {missing}"
            );
        }
        Ok(())
    }

    fn is_sdk_entity_parent(name: &str) -> bool {
        matches!(
            name,
            "IntentEntity" | "InsightIntent.AppIntentEntity" | "insightIntent.AppIntentEntity"
        )
    }

    fn attach_entities(&self, state: &mut InsightIntentCacheState) {
        let extends = state.entity_extends_map.clone();
        for entity_names in state.entity_owner_map.values_mut() {
            let mut cursor = 0;
            while cursor < entity_names.len() {
                if let Some(parent) = extends.get(&entity_names[cursor]) {
                    if !entity_names.contains(parent) {
                        entity_names.push(parent.clone());
                    }
                }
                cursor += 1;
            }
            entity_names.sort();
            entity_names.dedup();
        }
        for intent in &mut state.intents {
            let Some(intent_name) = self.intent_name(intent).map(str::to_string) else {
                continue;
            };
            let Some(entity_names) = state.entity_owner_map.get(&intent_name) else {
                continue;
            };
            let entities = entity_names
                .iter()
                .filter_map(|name| state.entity_map.get(name).cloned())
                .collect::<Vec<_>>();
            if !entities.is_empty() {
                if let Some(intent) = intent.as_object_mut() {
                    intent.insert("entities".to_string(), Value::Array(entities));
                }
            }
        }
    }

    fn merge_dependency_profiles(
        &self,
        dependencies: &[Map<String, Value>],
        bundle_name: &str,
        module_name: &str,
        preserve_empty_extract_field: bool,
        profile: &mut Map<String, Value>,
    ) -> Result<()> {
        let output = profile
            .entry("extractInsightIntents".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(output) = output.as_array_mut() else {
            anyhow::bail!("InsightIntent extractInsightIntents must be an array");
        };
        for dependency in dependencies {
            let Some(intents) = dependency
                .get("extractInsightIntents")
                .and_then(Value::as_array)
            else {
                continue;
            };
            for intent in intents {
                let mut intent = intent.clone();
                if let Some(intent) = intent.as_object_mut() {
                    intent.insert(
                        "bundleName".to_string(),
                        Value::String(bundle_name.to_string()),
                    );
                    intent.insert(
                        "moduleName".to_string(),
                        Value::String(module_name.to_string()),
                    );
                }
                output.push(intent);
            }
        }
        if output.is_empty() && !preserve_empty_extract_field {
            profile.remove("extractInsightIntents");
        }
        Ok(())
    }

    fn read_dependency_profiles(&self, paths: &[PathBuf]) -> Result<Vec<Map<String, Value>>> {
        let mut profiles = Vec::with_capacity(paths.len());
        for path in paths {
            if let Some(profile) = self
                .read_object(path)?
                .filter(|profile| !profile.is_empty())
            {
                profiles.push(profile);
            }
        }
        Ok(profiles)
    }

    fn validate_unique_intent_names(&self, profile: &Map<String, Value>) -> Result<()> {
        let mut names = std::collections::HashSet::new();
        for field in ["insightIntents", "extractInsightIntents"] {
            for intent in profile
                .get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(name) = self.intent_name(intent) else {
                    continue;
                };
                if !names.insert(name.to_string()) {
                    anyhow::bail!("[10110012] Duplicate intentName definitions found: {name}");
                }
            }
        }
        Ok(())
    }

    fn write_cache(&self, path: &Path, state: &InsightIntentCacheState) -> Result<()> {
        let entity_owner_map = state
            .entity_owner_map
            .iter()
            .map(|(name, values)| {
                (
                    name.clone(),
                    Value::Array(values.iter().cloned().map(Value::String).collect()),
                )
            })
            .collect::<Map<_, _>>();
        let entity_map = state
            .entity_map
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<Map<_, _>>();
        let entity_extends_map = state
            .entity_extends_map
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect::<Map<_, _>>();
        let source_file_path_map = state
            .source_file_path_map
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect::<Map<_, _>>();
        let heritage_class_set = state
            .heritage_class_set
            .iter()
            .map(|value| (value.clone(), Value::String(value.clone())))
            .collect::<Map<_, _>>();
        let entity_heritage_class_set = state
            .entity_heritage_class_set
            .iter()
            .map(|value| (value.clone(), Value::String(value.clone())))
            .collect::<Map<_, _>>();
        let cache = serde_json::json!({
            "extractInsightIntents": state.intents,
            "entityOwnerMap": entity_owner_map,
            "entityMap": entity_map,
            "heritageClassSet": heritage_class_set,
            "entityHeritageClassSet": entity_heritage_class_set,
            "entityExtendsMap": entity_extends_map,
            "sourceFilePathMap": source_file_path_map
        });
        self.write_pretty_json(path, &cache)
    }

    fn update_module_json(&self, path: &Path, has_intents: bool) -> Result<()> {
        if !path.is_file() {
            return Ok(());
        }
        let mut root = self
            .read_object(path)?
            .context("resource module.json must contain a JSON object")?;
        let module = root
            .get_mut("module")
            .and_then(Value::as_object_mut)
            .context("resource module.json must contain a module object")?;
        if has_intents {
            module.insert("hasInsightIntent".to_string(), Value::Bool(true));
        } else {
            module.remove("hasInsightIntent");
        }
        self.write_pretty_json(path, &Value::Object(root))
    }

    fn read_object(&self, path: &Path) -> Result<Option<Map<String, Value>>> {
        if !path.is_file() {
            return Ok(None);
        }
        let content = fs_err::read_to_string(path)
            .with_context(|| format!("read InsightIntent data {}", path.display()))?;
        let value: Value = json5::from_str(&content)
            .with_context(|| format!("parse InsightIntent data {}", path.display()))?;
        value
            .as_object()
            .cloned()
            .map(Some)
            .context("InsightIntent JSON root must be an object")
    }

    fn write_pretty_json(&self, path: &Path, value: &Value) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(value)?;
        fs_err::write(path, bytes)
            .with_context(|| format!("[10110025] write InsightIntent data {}", path.display()))
    }

    fn remove_if_exists(&self, path: &Path) -> Result<()> {
        if path.is_file() {
            fs_err::remove_file(path)?;
        }
        Ok(())
    }

    fn decorator_file<'a>(&self, value: &'a Value) -> Option<&'a str> {
        value.get("decoratorFile").and_then(Value::as_str)
    }

    fn intent_name<'a>(&self, value: &'a Value) -> Option<&'a str> {
        value.get("intentName").and_then(Value::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_build_without_intent_inputs_preserves_existing_profile_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let profile = temp.path().join("insight_intent.json");
        let cache = temp.path().join("missing/insight_compile_cache.json");
        let module_json = temp.path().join("module.json");
        let profile_source = r#"{"insightIntents":[{"intentName":"Manual"}]}"#;
        let module_source = r#"{"module":{"name":"entry","hasInsightIntent":true}}"#;
        fs_err::write(&profile, profile_source).expect("profile");
        fs_err::write(&module_json, module_source).expect("module json");

        let output = InsightIntentProfileProcessor::new()
            .process(InsightIntentProfileInput {
                profile_path: &profile,
                cache_path: &cache,
                module_json_path: &module_json,
                bundle_name: "bundle",
                module_name: "entry",
                modules: vec![InsightIntentModuleInput::default()],
                dependency_profiles: vec![],
            })
            .expect("source-owned afterBuildEnd gate");

        assert_eq!(output, InsightIntentProfileOutput::default());
        assert_eq!(fs_err::read_to_string(profile).unwrap(), profile_source);
        assert_eq!(fs_err::read_to_string(module_json).unwrap(), module_source);
        assert!(!cache.exists());
    }

    #[test]
    fn writes_profile_cache_and_module_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("entry/src/main/ets/Intent.ets");
        fs_err::create_dir_all(source.parent().unwrap()).expect("source parent");
        fs_err::write(&source, "class Intent {}").expect("source");
        let profile = temp
            .path()
            .join("res/resources/base/profile/insight_intent.json");
        let cache = temp.path().join("cache/insight_compile_cache.json");
        let module_json = temp.path().join("res/module.json");
        fs_err::create_dir_all(module_json.parent().unwrap()).expect("module parent");
        fs_err::write(&module_json, r#"{"module":{"name":"entry"}}"#).expect("module json");
        let intent = InsightIntentInput {
            data: serde_json::from_value::<Map<String, Value>>(serde_json::json!({
                "decoratorFile": "@normalized:N&entry&com.example&entry/src/main/ets/Intent&1.0.0",
                "decoratorClass": "Intent",
                "decoratorType": "@InsightIntentLink",
                "intentName": "Play",
                "domain": "Music",
                "intentVersion": "1",
                "displayName": "Play",
                "uri": "/play"
            }))
            .expect("intent object"),
            entity_type_names: vec![],
        };
        let output = InsightIntentProfileProcessor::new()
            .process(InsightIntentProfileInput {
                profile_path: &profile,
                cache_path: &cache,
                module_json_path: &module_json,
                bundle_name: "com.example",
                module_name: "entry",
                modules: vec![InsightIntentModuleInput {
                    source_file_path: source,
                    decorator_file:
                        "@normalized:N&entry&com.example&entry/src/main/ets/Intent&1.0.0"
                            .to_string(),
                    intents: vec![intent],
                    entities: vec![],
                }],
                dependency_profiles: vec![],
            })
            .expect("process profile");

        assert_eq!(output.extract_intent_count, 1);
        assert!(profile.is_file());
        assert!(cache.is_file());
        let module: Value =
            serde_json::from_str(&fs_err::read_to_string(module_json).expect("read module json"))
                .expect("parse module json");
        assert_eq!(module["module"]["hasInsightIntent"], true);
    }

    #[test]
    fn merges_entities_and_dependency_intents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("Intent.ets");
        fs_err::write(&source, "class Intent {}").expect("source");
        let dependency = temp.path().join("dependency.json");
        fs_err::write(
            &dependency,
            r#"{"extractInsightIntents":[{"intentName":"Dependency","bundleName":"old","moduleName":"har"}]}"#,
        )
        .expect("dependency");
        let profile = temp.path().join("insight_intent.json");
        let cache = temp.path().join("insight_compile_cache.json");
        let mut intent = Map::new();
        intent.insert("intentName".to_string(), Value::String("Host".to_string()));
        intent.insert(
            "decoratorFile".to_string(),
            Value::String("host".to_string()),
        );
        let mut entity = Map::new();
        entity.insert("className".to_string(), Value::String("Song".to_string()));
        entity.insert(
            "decoratorFile".to_string(),
            Value::String("host".to_string()),
        );

        InsightIntentProfileProcessor::new()
            .process(InsightIntentProfileInput {
                profile_path: &profile,
                cache_path: &cache,
                module_json_path: &temp.path().join("missing-module.json"),
                bundle_name: "com.host",
                module_name: "entry",
                modules: vec![InsightIntentModuleInput {
                    source_file_path: source,
                    decorator_file: "host".to_string(),
                    intents: vec![InsightIntentInput {
                        data: intent,
                        entity_type_names: vec!["Song".to_string()],
                    }],
                    entities: vec![InsightIntentEntityInput {
                        class_name: "Song".to_string(),
                        data: entity,
                        parent_class_name: None,
                    }],
                }],
                dependency_profiles: vec![dependency],
            })
            .expect("process profile");
        let profile: Value =
            serde_json::from_str(&fs_err::read_to_string(profile).expect("read profile"))
                .expect("parse profile");
        assert_eq!(
            profile["extractInsightIntents"].as_array().unwrap().len(),
            2
        );
        assert_eq!(
            profile["extractInsightIntents"][0]["entities"][0]["className"],
            "Song"
        );
        assert_eq!(
            profile["extractInsightIntents"][1]["bundleName"],
            "com.host"
        );
        assert_eq!(profile["extractInsightIntents"][1]["moduleName"], "entry");
    }

    #[test]
    fn recompiled_source_without_intents_retains_source_empty_profile_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("Intent.ets");
        fs_err::write(&source, "class Intent {}").expect("source");
        let profile = temp.path().join("insight_intent.json");
        let cache = temp.path().join("insight_compile_cache.json");
        let decorator_file = "@normalized:N&entry&bundle&Intent&1.0.0";
        let mut data = Map::new();
        data.insert(
            "decoratorFile".to_string(),
            Value::String(decorator_file.to_string()),
        );
        data.insert("intentName".to_string(), Value::String("Play".to_string()));
        let processor = InsightIntentProfileProcessor::new();
        processor
            .process(InsightIntentProfileInput {
                profile_path: &profile,
                cache_path: &cache,
                module_json_path: &temp.path().join("module.json"),
                bundle_name: "bundle",
                module_name: "entry",
                modules: vec![InsightIntentModuleInput {
                    source_file_path: source.clone(),
                    decorator_file: decorator_file.to_string(),
                    intents: vec![InsightIntentInput {
                        data,
                        entity_type_names: vec![],
                    }],
                    entities: vec![],
                }],
                dependency_profiles: vec![],
            })
            .expect("initial intent profile");
        assert!(profile.is_file());

        processor
            .process(InsightIntentProfileInput {
                profile_path: &profile,
                cache_path: &cache,
                module_json_path: &temp.path().join("module.json"),
                bundle_name: "bundle",
                module_name: "entry",
                modules: vec![InsightIntentModuleInput {
                    source_file_path: source,
                    decorator_file: decorator_file.to_string(),
                    intents: vec![],
                    entities: vec![],
                }],
                dependency_profiles: vec![],
            })
            .expect("recompiled intent-free source");

        let profile_data: Value =
            serde_json::from_slice(&fs_err::read(&profile).expect("updated profile"))
                .expect("profile json");
        let cache_data: Value =
            serde_json::from_slice(&fs_err::read(&cache).expect("cache")).expect("cache json");
        assert_eq!(profile_data["extractInsightIntents"], serde_json::json!([]));
        assert_eq!(cache_data["extractInsightIntents"], serde_json::json!([]));
    }

    #[test]
    fn writes_entity_inheritance_sets_in_sdk_cache_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("Entity.ets");
        fs_err::write(&source, "class Entity {}").expect("source");
        let profile = temp.path().join("insight_intent.json");
        let cache = temp.path().join("insight_compile_cache.json");
        let decorator_file = "@normalized:N&entry&bundle&Entity&1.0.0";
        let mut entity = Map::new();
        entity.insert("className".to_string(), Value::String("Song".to_string()));
        entity.insert(
            "decoratorFile".to_string(),
            Value::String(decorator_file.to_string()),
        );
        entity.insert(
            "parentClassName".to_string(),
            Value::String("IntentEntity".to_string()),
        );
        let mut intent = Map::new();
        intent.insert("intentName".to_string(), Value::String("Play".to_string()));
        intent.insert(
            "decoratorFile".to_string(),
            Value::String(decorator_file.to_string()),
        );

        InsightIntentProfileProcessor::new()
            .process(InsightIntentProfileInput {
                profile_path: &profile,
                cache_path: &cache,
                module_json_path: &temp.path().join("module.json"),
                bundle_name: "bundle",
                module_name: "entry",
                modules: vec![InsightIntentModuleInput {
                    source_file_path: source,
                    decorator_file: decorator_file.to_string(),
                    intents: vec![InsightIntentInput {
                        data: intent,
                        entity_type_names: vec!["Song".to_string()],
                    }],
                    entities: vec![InsightIntentEntityInput {
                        class_name: "Song".to_string(),
                        data: entity,
                        parent_class_name: Some("IntentEntity".to_string()),
                    }],
                }],
                dependency_profiles: vec![],
            })
            .expect("entity cache");
        let cache: Value =
            serde_json::from_str(&fs_err::read_to_string(cache).expect("read cache"))
                .expect("parse cache");
        assert_eq!(
            cache["heritageClassSet"][format!("Song_{decorator_file}")],
            format!("Song_{decorator_file}")
        );
        assert_eq!(
            cache["entityHeritageClassSet"]["IntentEntity_sdk"],
            "IntentEntity_sdk"
        );
    }
}
