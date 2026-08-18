use std::collections::HashSet;

use indexmap::IndexMap;
use napi::bindgen_prelude::{
  Array, Env, FnArgs, Function, JsObjectValue, Object, Unknown, ValueType,
};
use napi::{JsValue, Result};
use serde::Deserialize;
use serde_json::{json, Map, Value};

/// Reads the live official Hvigor task objects at the NAPI boundary. Task
/// implementations must remain in V8 because plugins own their callbacks, but
/// descriptor/declaration projection belongs beside the Rust task graph that
/// consumes it.
pub(crate) struct RuntimeTaskNapiInspector<'env> {
  env: Env,
  core_task: Object<'env>,
  task_context: Object<'env>,
  class_incremental_task: bool,
}

/// Projects the complete live official node/task inventory in one native
/// boundary. JavaScript synchronizes LazyTaskContainer first because plugin
/// objects live in V8; Rust reads the official task container directly and
/// owns descriptor construction and deterministic ordering consumed by the
/// task graph. No duplicate JavaScript task-record map is maintained.
pub(crate) struct RuntimeTaskInventoryNapiInspector<'env> {
  env: Env,
  nodes: Array<'env>,
  include_catalog: bool,
}

impl<'env> RuntimeTaskInventoryNapiInspector<'env> {
  pub(crate) fn new(env: Env, nodes: Array<'env>, include_catalog: bool) -> Self {
    Self {
      env,
      nodes,
      include_catalog,
    }
  }

  /// `resolve_catalog` answers catalog requests from the Rust session plan
  /// instead of the JavaScript catalog facade. Calling the facade here would
  /// re-enter the native `plan()` boundary while `inspect_runtime_task_model`
  /// still holds the mutable borrow of the runtime session, which NAPI
  /// rejects with a conflicting-borrow error.
  pub(crate) fn inspect_model<F>(&self, resolve_catalog: F) -> Result<Value>
  where
    F: Fn(&str, bool) -> Result<super::catalog::RuntimeTaskCatalogSnapshot>,
  {
    let mut descriptors = Vec::new();
    let mut command_nodes = Vec::new();
    let mut catalogs = Vec::new();
    let mut project_custom_tasks = Vec::new();
    let mut module_custom_tasks = Map::new();
    for node_index in 0..self.nodes.len() {
      let node = self.nodes.get::<Object<'env>>(node_index)?.ok_or_else(|| {
        napi::Error::from_reason(format!("Hvigor runtime node {node_index} is not an object"))
      })?;
      let node_name = Self::required_property::<String>(&node, "name")?;
      let project_task = node.get::<bool>("isProject")?.unwrap_or(false);
      let catalog = resolve_catalog(&node_name, project_task)?;
      catalogs.push(catalog.request.clone());
      let core = Self::required_property::<Object<'env>>(&node, "coreWrapper")?;
      let task_values = Self::required_method(&core, "getAllTasks")?;
      let task_values = self.array_from(task_values)?;
      let mut custom_tasks = Vec::new();
      let mut custom_sync_task_paths = Vec::new();
      let mut realized_task_names = HashSet::with_capacity(task_values.len() as usize);
      let mut realization_order = 0;
      for task_index in 0..task_values.len() {
        let core_task = task_values
          .get::<Object<'env>>(task_index)?
          .ok_or_else(|| {
            napi::Error::from_reason(format!(
              "Hvigor runtime node {node_name} task {task_index} is not an object"
            ))
          })?;
        let task_name = Self::required_string_method(&core_task, "getName")?;
        realized_task_names.insert(task_name.clone());
        let task_context = core_task
          .get::<Object<'env>>("taskContext")?
          .unwrap_or_else(|| core_task.clone());
        let catalog_definition = catalog.definition(&task_name);
        let explicitly_built_in = core_task
          .get::<Value>("__arkdownBuiltInDefinition")?
          .is_some_and(|value| !value.is_null());
        let user_task = core_task.get::<bool>("__arkdownUserTask")?.unwrap_or(false);
        let built_in = explicitly_built_in || (!user_task && catalog_definition.is_some());
        let task_path = Self::required_string_method(&core_task, "getPath")?;
        if !built_in && core_task.get::<bool>("__arkdownSyncTask")?.unwrap_or(false) {
          custom_sync_task_paths.push(task_path.clone());
        }
        let definition = core_task.get::<Object<'env>>("task")?;
        let class_incremental_task = Self::has_function(&core_task, "declareInputs")?
          && Self::has_function(&core_task, "declareInputFiles")?
          && Self::has_function(&core_task, "declareOutputFiles")?;
        let inspector =
          RuntimeTaskNapiInspector::new(self.env, core_task, task_context, class_incremental_task);
        let descriptor = inspector.descriptor(
          task_path,
          node_name.clone(),
          built_in,
          project_task,
          catalog_definition,
          definition.as_ref().is_some_and(|value| {
            Self::has_values(value, "dependencies") || Self::has_values(value, "postDependencies")
          }),
          realization_order,
        )?;
        if !built_in {
          custom_tasks.push(json!({
            "taskName": task_name,
            "group": descriptor.get("group").cloned().unwrap_or(Value::String("Customize".to_string())),
            "description": descriptor.get("description").cloned().unwrap_or(Value::String(String::new())),
            "sourcePluginId": inspector.optional_string_method("getSourcePluginId")?.unwrap_or_default(),
          }));
        }
        descriptors.push(descriptor);
        realization_order += 1;
      }

      let creations = Self::required_method(&core, "getAllTasksCA")?;
      let creations = self.array_from(creations)?;
      for creation_index in 0..creations.len() {
        let creation = creations
          .get::<Object<'env>>(creation_index)?
          .ok_or_else(|| {
            napi::Error::from_reason(format!(
              "Hvigor runtime node {node_name} lazy task {creation_index} is not an object"
            ))
          })?;
        let details = Self::required_property::<Object<'env>>(&creation, "details")?;
        let task_name = Self::required_property::<String>(&details, "name")?;
        if realized_task_names.contains(&task_name) {
          continue;
        }
        let definition = creation.get::<Value>("__arkdownBuiltInDefinition")?;
        if definition.is_some() && !self.include_catalog {
          continue;
        }
        let dependencies = Self::value_list(creation.get::<Value>("depends")?)
          .into_iter()
          .chain(Self::value_list(
            creation.get::<Value>("dependsDefinedByUser")?,
          ))
          .collect::<Vec<_>>();
        let group = details
          .get::<String>("group")?
          .unwrap_or_else(|| "Customize".to_string());
        let description = details.get::<String>("description")?.unwrap_or_default();
        let source_plugin_id = details.get::<String>("sourcePluginId")?.unwrap_or_default();
        let descriptor = RuntimeLazyTaskDescriptorPlanner::from_value(json!({
          "nodeName": node_name,
          "taskName": task_name,
          "builtIn": definition.is_some(),
          "projectTask": project_task,
          "group": group,
          "description": description,
          "catalogOrder": definition.as_ref().and_then(|value| value.get("catalogOrder")),
          "realizationOrder": realization_order,
          "dependencies": dependencies,
        }))?
        .plan();
        if definition.is_none() {
          custom_tasks.push(json!({
            "taskName": task_name,
            "group": group,
            "description": description,
            "sourcePluginId": source_plugin_id,
          }));
        }
        descriptors.push(descriptor);
        realization_order += 1;
      }

      let children = node
        .get::<Array<'env>>("children")?
        .map(|children| {
          (0..children.len())
            .map(|index| {
              children
                .get::<Object<'env>>(index)?
                .ok_or_else(|| {
                  napi::Error::from_reason(format!(
                    "Hvigor runtime node {node_name} child {index} is not an object"
                  ))
                })
                .and_then(|child| Self::required_property::<String>(&child, "name"))
            })
            .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
      let kind = node
        .get::<Value>("kind")?
        .filter(|kind| !kind.is_null())
        .unwrap_or(Value::Null);
      command_nodes.push(json!({
        "name": node_name,
        "kind": kind,
        "projectTask": project_task,
        "childNames": children,
        "customSyncTaskPaths": custom_sync_task_paths,
      }));
      if project_task {
        project_custom_tasks = custom_tasks;
      } else {
        module_custom_tasks.insert(node_name, Value::Array(custom_tasks));
      }
    }
    descriptors.sort_by(|left, right| {
      left
        .get("path")
        .and_then(Value::as_str)
        .cmp(&right.get("path").and_then(Value::as_str))
    });
    Ok(json!({
      "nodes": command_nodes,
      "catalogs": catalogs,
      "removedTaskPaths": [],
      "tasks": descriptors,
      "projectCustomTasks": project_custom_tasks,
      "moduleCustomTasks": module_custom_tasks,
    }))
  }

  fn required_property<T>(object: &Object<'env>, name: &str) -> Result<T>
  where
    T: napi::bindgen_prelude::FromNapiValue + napi::bindgen_prelude::ValidateNapiValue,
  {
    object
      .get::<T>(name)?
      .ok_or_else(|| napi::Error::from_reason(format!("Hvigor runtime object is missing {name}")))
  }

  fn required_method(object: &Object<'env>, name: &str) -> Result<Unknown<'env>> {
    object
      .get::<Function<(), Unknown<'env>>>(name)?
      .ok_or_else(|| {
        napi::Error::from_reason(format!("Hvigor runtime object is missing {name}()"))
      })?
      .apply(object, ())
  }

  fn required_string_method(object: &Object<'env>, name: &str) -> Result<String> {
    object
      .get::<Function<(), String>>(name)?
      .ok_or_else(|| {
        napi::Error::from_reason(format!("Hvigor runtime object is missing {name}()"))
      })?
      .apply(object, ())
  }

  fn has_values(object: &Object<'env>, field: &str) -> bool {
    object
      .get::<Unknown<'env>>(field)
      .ok()
      .flatten()
      .is_some_and(|value| match value.get_type() {
        Ok(ValueType::Undefined | ValueType::Null) | Err(_) => false,
        Ok(_) => value
          .is_array()
          .ok()
          .filter(|is_array| *is_array)
          .and_then(|_| unsafe { value.cast::<Array<'env>>().ok() })
          .is_none_or(|array| array.len() > 0),
      })
  }

  fn has_function(object: &Object<'env>, name: &str) -> Result<bool> {
    Ok(object.get::<Unknown<'env>>(name)?.is_some_and(|value| {
      value
        .get_type()
        .is_ok_and(|kind| kind == ValueType::Function)
    }))
  }

  fn value_list(value: Option<Value>) -> Vec<Value> {
    match value {
      None | Some(Value::Null) => Vec::new(),
      Some(Value::Array(values)) => values,
      Some(value) => vec![value],
    }
  }

  fn array_from(&self, value: Unknown<'env>) -> Result<Array<'env>> {
    let global = self.env.get_global()?;
    let array_constructor: Function<'env, (), Unknown<'env>> =
      global.get_named_property("Array")?;
    let from = array_constructor
      .get_named_property::<Function<FnArgs<(Unknown<'env>,)>, Array<'env>>>("from")?;
    from.apply(array_constructor, FnArgs::from((value,)))
  }
}

impl<'env> RuntimeTaskNapiInspector<'env> {
  pub(crate) fn new(
    env: Env,
    core_task: Object<'env>,
    task_context: Object<'env>,
    class_incremental_task: bool,
  ) -> Self {
    Self {
      env,
      core_task,
      task_context,
      class_incremental_task,
    }
  }

  pub(crate) fn descriptor(
    &self,
    path: String,
    node_name: String,
    built_in: bool,
    project_task: bool,
    catalog_definition: Option<Value>,
    graph_bound_custom: bool,
    realization_order: u32,
  ) -> Result<Value> {
    let task_name = self.required_string_method("getName")?;
    let group = self
      .optional_string_method("getGroup")?
      .or_else(|| Self::catalog_string(&catalog_definition, "group"))
      .unwrap_or_else(|| if built_in { "Other" } else { "Customize" }.to_string());
    let description = self
      .optional_string_method("getDescription")?
      .or_else(|| Self::catalog_string(&catalog_definition, "description"))
      .unwrap_or_default();
    let catalog_order = catalog_definition
      .as_ref()
      .and_then(|definition| definition.get("catalogOrder"))
      .cloned();
    let mut descriptor = json!({
      "path": path,
      "nodeName": node_name,
      "taskName": task_name,
      "builtIn": built_in,
      "projectTask": project_task,
      "lazy": false,
      "realized": true,
      "group": group,
      "description": description,
      "realizationOrder": realization_order,
      "graphBoundCustom": graph_bound_custom,
      "dependencies": self.string_array_method("getDependsOn")?,
      "userDependencies": [],
    });
    if let Some(catalog_order) = catalog_order {
      descriptor["catalogOrder"] = catalog_order;
    }
    Self::extend_object(&mut descriptor, self.state()?);
    Ok(descriptor)
  }

  pub(crate) fn state(&self) -> Result<Value> {
    let mut state = json!({
      "enabled": self.enabled()?,
      "trackState": self.track_state()?,
      "refreshDeclarations": self.class_incremental_task,
      "hasBeforeRun": self.nested_array_is_non_empty("getBeforeRunStack", "stack")?,
      "hasAfterRun": self.nested_array_is_non_empty("getAfterRunQueue", "elements")?,
      "hasFailed": Self::has_function(&self.task_context, "onFailed")?,
      "hasTaskShouldDo": Self::has_function(&self.task_context, "taskShouldDo")?,
    });
    Self::extend_object(&mut state, self.declarations()?);
    Ok(state)
  }

  pub(crate) fn declarations(&self) -> Result<Value> {
    if !self.class_incremental_task {
      return Ok(json!({ "inputProperties": [], "inputFiles": [], "outputFiles": [] }));
    }
    Ok(json!({
      "inputProperties": self.property_records(self.required_method("declareInputs")?)?,
      "inputFiles": self.file_declarations(self.required_method("declareInputFiles")?)?,
      "outputFiles": self.file_declarations(self.required_method("declareOutputFiles")?)?,
    }))
  }

  fn enabled(&self) -> Result<bool> {
    let value = self.required_method("getEnabled")?;
    Ok(!(value.get_type()? == ValueType::Boolean && unsafe { value.cast::<bool>()? } == false))
  }

  fn track_state(&self) -> Result<bool> {
    let Some(status) = self.core_task.get::<Object<'env>>("taskExecutedStatus")? else {
      return Ok(true);
    };
    Ok(status.get::<Unknown<'env>>("unTrackStateReason")?.is_none())
  }

  fn nested_array_is_non_empty(&self, method: &str, field: &str) -> Result<bool> {
    let Some(value) = self.optional_method(method)? else {
      return Ok(false);
    };
    let object = unsafe { value.cast::<Object<'env>>()? };
    Ok(
      object
        .get::<Array<'env>>(field)?
        .is_some_and(|array| array.len() > 0),
    )
  }

  fn property_records(&self, entries: Unknown<'env>) -> Result<Vec<Value>> {
    let entries = self.array_from(entries)?;
    let mut records = Vec::with_capacity(entries.len() as usize);
    for index in 0..entries.len() {
      let entry = entries.get::<Array<'env>>(index)?.ok_or_else(|| {
        napi::Error::from_reason(format!(
          "Hvigor task input entry {index} is not a key/value pair"
        ))
      })?;
      let name = entry.get::<Unknown<'env>>(0)?.ok_or_else(|| {
        napi::Error::from_reason(format!("Hvigor task input entry {index} has no name"))
      })?;
      let value = entry.get::<Unknown<'env>>(1)?.ok_or_else(|| {
        napi::Error::from_reason(format!("Hvigor task input entry {index} has no value"))
      })?;
      records.push(json!({
        "name": self.js_string(name)?,
        "value": self.js_string(value)?,
        "valueType": Self::javascript_type(value.get_type()?),
      }));
    }
    Ok(records)
  }

  fn file_declarations(&self, source: Unknown<'env>) -> Result<Vec<Value>> {
    let source = if source.get_type()? == ValueType::Object && !source.is_array()? {
      let object = unsafe { source.cast::<Object<'env>>()? };
      if let Some(collect) = object.get::<Function<(), Unknown<'env>>>("collect")? {
        collect.apply(&object, ())?
      } else {
        source
      }
    } else {
      source
    };

    let mut declarations = IndexMap::<String, Value>::new();
    if source.is_array()? {
      let source = unsafe { source.cast::<Array<'env>>()? };
      for index in 0..source.len() {
        let declaration = source.get::<Unknown<'env>>(index)?.ok_or_else(|| {
          napi::Error::from_reason(format!("Hvigor task file declaration {index} is missing"))
        })?;
        let (path, options) = if declaration.get_type()? == ValueType::String {
          (unsafe { declaration.cast::<String>()? }, None)
        } else {
          let options = unsafe { declaration.cast::<Object<'env>>()? };
          let path = options.get::<String>("path")?.ok_or_else(|| {
            napi::Error::from_reason(format!("Hvigor task file declaration {index} has no path"))
          })?;
          (path, Some(options))
        };
        declarations.insert(path.clone(), self.file_declaration(path, options.as_ref())?);
      }
    } else {
      let entries = self.array_from(source)?;
      for index in 0..entries.len() {
        let entry = entries.get::<Array<'env>>(index)?.ok_or_else(|| {
          napi::Error::from_reason(format!(
            "Hvigor task file entry {index} is not a key/value pair"
          ))
        })?;
        let path = entry
          .get::<Unknown<'env>>(0)?
          .ok_or_else(|| napi::Error::from_reason("Hvigor task file entry has no path"))?;
        let path = self.js_string(path)?;
        let options = entry
          .get::<Unknown<'env>>(1)?
          .filter(|value| value.get_type().is_ok_and(|kind| kind == ValueType::Object))
          .map(|value| unsafe { value.cast::<Object<'env>>() })
          .transpose()?;
        declarations.insert(path.clone(), self.file_declaration(path, options.as_ref())?);
      }
    }
    Ok(declarations.into_values().collect())
  }

  fn file_declaration(&self, path: String, options: Option<&Object<'env>>) -> Result<Value> {
    let mut declaration = Map::from_iter([("path".to_string(), Value::String(path))]);
    let Some(options) = options else {
      return Ok(Value::Object(declaration));
    };
    if let Some(value) = options.get::<Unknown<'env>>("isDirectory")? {
      declaration.insert(
        "isDirectory".to_string(),
        Value::Bool(self.js_boolean(value)?),
      );
    }
    for (source, target) in [
      ("depth", "depth"),
      ("fileSnapShotHashValue", "fileSnapShotHashValue"),
    ] {
      if let Some(value) = options.get::<Value>(source)? {
        declaration.insert(target.to_string(), value);
      }
    }
    if let Some(test) = options.get::<Object<'env>>("test")? {
      if let Some(source) = test.get::<String>("source")? {
        declaration.insert(
          "test".to_string(),
          json!({ "source": source, "flags": test.get::<String>("flags")?.unwrap_or_default() }),
        );
      }
    }
    Ok(Value::Object(declaration))
  }

  fn string_array_method(&self, name: &str) -> Result<Vec<String>> {
    let value = self.required_method(name)?;
    let values = self.array_from(value)?;
    let mut unique = IndexMap::<String, ()>::new();
    for index in 0..values.len() {
      let value = values.get::<String>(index)?.ok_or_else(|| {
        napi::Error::from_reason(format!("Hvigor task {name}()[{index}] is not a string"))
      })?;
      unique.entry(value).or_insert(());
    }
    Ok(unique.into_keys().collect())
  }

  fn required_string_method(&self, name: &str) -> Result<String> {
    let value = self.required_method(name)?;
    if value.get_type()? != ValueType::String {
      return Err(napi::Error::from_reason(format!(
        "Hvigor task {name}() did not return a string"
      )));
    }
    unsafe { value.cast::<String>() }
  }

  fn optional_string_method(&self, name: &str) -> Result<Option<String>> {
    let Some(value) = self.optional_method(name)? else {
      return Ok(None);
    };
    match value.get_type()? {
      ValueType::Null | ValueType::Undefined => Ok(None),
      ValueType::String => unsafe { value.cast::<String>().map(Some) },
      _ => Err(napi::Error::from_reason(format!(
        "Hvigor task {name}() did not return a string"
      ))),
    }
  }

  fn required_method(&self, name: &str) -> Result<Unknown<'env>> {
    self
      .optional_method(name)?
      .ok_or_else(|| napi::Error::from_reason(format!("Hvigor task is missing required {name}()")))
  }

  fn optional_method(&self, name: &str) -> Result<Option<Unknown<'env>>> {
    self
      .core_task
      .get::<Function<(), Unknown<'env>>>(name)?
      .map(|method| method.apply(&self.core_task, ()))
      .transpose()
  }

  fn array_from(&self, value: Unknown<'env>) -> Result<Array<'env>> {
    let global = self.env.get_global()?;
    let array_constructor: Function<'env, (), Unknown<'env>> =
      global.get_named_property("Array")?;
    let from = array_constructor
      .get_named_property::<Function<FnArgs<(Unknown<'env>,)>, Array<'env>>>("from")?;
    from.apply(array_constructor, FnArgs::from((value,)))
  }

  fn js_string(&self, value: Unknown<'env>) -> Result<String> {
    let function: Function<FnArgs<(Unknown<'env>,)>, String> =
      self.env.get_global()?.get_named_property("String")?;
    function.call(FnArgs::from((value,)))
  }

  fn js_boolean(&self, value: Unknown<'env>) -> Result<bool> {
    let function: Function<FnArgs<(Unknown<'env>,)>, bool> =
      self.env.get_global()?.get_named_property("Boolean")?;
    function.call(FnArgs::from((value,)))
  }

  fn has_function(object: &Object<'env>, name: &str) -> Result<bool> {
    Ok(object.get::<Unknown<'env>>(name)?.is_some_and(|value| {
      value
        .get_type()
        .is_ok_and(|kind| kind == ValueType::Function)
    }))
  }

  fn catalog_string(definition: &Option<Value>, field: &str) -> Option<String> {
    definition
      .as_ref()
      .and_then(|value| value.get(field))
      .and_then(Value::as_str)
      .map(str::to_string)
  }

  fn javascript_type(value_type: ValueType) -> &'static str {
    match value_type {
      ValueType::Undefined => "undefined",
      ValueType::Null | ValueType::Object | ValueType::External => "object",
      ValueType::Boolean => "boolean",
      ValueType::Number => "number",
      ValueType::String => "string",
      ValueType::Symbol => "symbol",
      ValueType::Function => "function",
      // With the addon's N-API 4 compatibility floor, napi-rs reports the
      // later BigInt type through Unknown.
      ValueType::Unknown => "bigint",
    }
  }

  fn extend_object(target: &mut Value, source: Value) {
    let Some(target) = target.as_object_mut() else {
      return;
    };
    if let Value::Object(source) = source {
      target.extend(source);
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeLazyTaskDescriptorRequest {
  node_name: String,
  project_task: bool,
  task_name: String,
  built_in: bool,
  #[serde(default)]
  group: Option<String>,
  #[serde(default)]
  description: String,
  #[serde(default)]
  catalog_order: Option<u32>,
  realization_order: u32,
  #[serde(default)]
  dependencies: Vec<Value>,
}

pub(crate) struct RuntimeLazyTaskDescriptorPlanner {
  request: RuntimeLazyTaskDescriptorRequest,
}

impl RuntimeLazyTaskDescriptorPlanner {
  pub(crate) fn from_value(value: Value) -> Result<Self> {
    serde_json::from_value(value)
      .map(|request| Self { request })
      .map_err(|error| napi::Error::from_reason(format!("invalid lazy Hvigor task: {error}")))
  }

  pub(crate) fn plan(self) -> Value {
    let request = self.request;
    let path = if request.project_task {
      format!(":{}", request.task_name)
    } else {
      format!("{}:{}", request.node_name, request.task_name)
    };
    let dependencies = request
      .dependencies
      .into_iter()
      .filter_map(|value| value.as_str().map(str::to_string))
      .map(|dependency| {
        if dependency.starts_with(':') || dependency.contains(':') {
          dependency
        } else if request.project_task {
          format!(":{dependency}")
        } else {
          format!("{}:{dependency}", request.node_name)
        }
      })
      .collect::<Vec<_>>();
    let mut descriptor = json!({
      "path": path,
      "nodeName": request.node_name,
      "taskName": request.task_name,
      "builtIn": request.built_in,
      "projectTask": request.project_task,
      "lazy": !request.built_in,
      "realized": false,
      "group": request.group.unwrap_or_else(|| if request.built_in { "Other" } else { "Customize" }.to_string()),
      "description": request.description,
      "realizationOrder": request.realization_order,
      "enabled": true,
      "trackState": true,
      "refreshDeclarations": false,
      "hasBeforeRun": false,
      "hasAfterRun": false,
      "hasFailed": false,
      "hasTaskShouldDo": false,
      "graphBoundCustom": !request.built_in && !dependencies.is_empty(),
      "dependencies": dependencies,
      "userDependencies": [],
      "inputProperties": [],
      "inputFiles": [],
      "outputFiles": [],
    });
    if let Some(catalog_order) = request.catalog_order {
      descriptor["catalogOrder"] = Value::from(catalog_order);
    }
    descriptor
  }
}
