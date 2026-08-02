use arkdown::plugins::hvigor_compile::{
  HvigorCompileOrderCallbackError, HvigorCompilePluginDescriptor, HvigorCompilePluginDiagnostic,
  HvigorCompilePluginOrder, HvigorCompilePluginPlacement, HvigorCompilePluginPlanConfig,
  HvigorCompilePluginPlanner, HvigorPluginAttributeShape, HvigorPluginShape,
  HvigorPluginShapeValidator, HvigorPluginValidationOutcome, HvigorPluginValueKind,
  HvigorPluginValueShape, RolldownPackagePluginDescriptor, RolldownPackagePluginPlacement,
  RolldownPackagePluginPlanner,
};
use napi::bindgen_prelude::*;
use napi::JsValue;
use serde::Serialize;

type OrderCallback<'env> = Function<'env, FnArgs<(Object<'env>,)>, Unknown<'env>>;

struct OrderCallbackBridge<'env> {
  env: Env,
  callbacks: Vec<Option<OrderCallback<'env>>>,
}

impl<'env> OrderCallbackBridge<'env> {
  fn new(env: Env, callbacks: Vec<Option<OrderCallback<'env>>>) -> Self {
    Self { env, callbacks }
  }

  fn invoke(
    &self,
    source_index: usize,
    names: &[(String, usize)],
  ) -> std::result::Result<f64, HvigorCompileOrderCallbackError> {
    let callback = self
      .callbacks
      .get(source_index)
      .and_then(Option::as_ref)
      .ok_or_else(|| {
        HvigorCompileOrderCallbackError::Threw("native callback bridge is missing".to_string())
      })?;
    let mut name_to_index = Object::new(&self.env)
      .map_err(|error| HvigorCompileOrderCallbackError::Threw(error.reason.clone()))?;
    for (name, index) in names {
      let index = u32::try_from(*index).map_err(|_| {
        HvigorCompileOrderCallbackError::Threw("plugin name index exceeds u32".to_string())
      })?;
      name_to_index
        .set(name, index)
        .map_err(|error| HvigorCompileOrderCallbackError::Threw(error.reason.clone()))?;
    }
    let result = callback
      .call(FnArgs {
        data: (name_to_index,),
      })
      .map_err(|error| HvigorCompileOrderCallbackError::Threw(error.reason.clone()))?;
    Self::callback_number(&result)
  }

  fn callback_number(
    value: &Unknown<'env>,
  ) -> std::result::Result<f64, HvigorCompileOrderCallbackError> {
    if value
      .get_type()
      .map_err(|error| HvigorCompileOrderCallbackError::Threw(error.reason.clone()))?
      == ValueType::Number
    {
      return unsafe { value.cast::<f64>() }
        .map_err(|error| HvigorCompileOrderCallbackError::Threw(error.reason.clone()));
    }
    Err(HvigorCompileOrderCallbackError::InvalidReturn(
      Self::callback_value_description(value),
    ))
  }

  fn callback_value_description(value: &Unknown<'env>) -> String {
    let value_type = value.get_type().unwrap_or(ValueType::Unknown);
    match value_type {
      ValueType::Undefined => "undefined".to_string(),
      ValueType::Function => "function".to_string(),
      ValueType::Symbol => "symbol".to_string(),
      ValueType::Null => "null".to_string(),
      ValueType::Boolean => unsafe { value.cast::<bool>() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "boolean".to_string()),
      ValueType::String => unsafe { value.cast::<String>() }
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .unwrap_or_else(|| "string".to_string()),
      _ => value_type.to_string(),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorCompilePluginPlacementResponse {
  plugin_index: u32,
  order_index: u32,
  anchor_rank: u32,
  anchor_sequence: u32,
}

impl TryFrom<HvigorCompilePluginPlacement> for HvigorCompilePluginPlacementResponse {
  type Error = napi::Error;

  fn try_from(placement: HvigorCompilePluginPlacement) -> napi::Result<Self> {
    Ok(Self {
      plugin_index: u32::try_from(placement.source_index)
        .map_err(|_| napi::Error::from_reason("compile plugin index exceeds u32"))?,
      order_index: u32::try_from(placement.order_index)
        .map_err(|_| napi::Error::from_reason("compile plugin order index exceeds u32"))?,
      anchor_rank: u32::from(placement.anchor_rank),
      anchor_sequence: placement.anchor_sequence,
    })
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorCompilePluginDiagnosticResponse {
  code: &'static str,
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  plugin_name: Option<String>,
}

impl From<HvigorCompilePluginDiagnostic> for HvigorCompilePluginDiagnosticResponse {
  fn from(diagnostic: HvigorCompilePluginDiagnostic) -> Self {
    Self {
      code: diagnostic.code,
      message: diagnostic.message,
      plugin_name: diagnostic.plugin_name,
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorCompilePluginPlanResponse {
  placements: Vec<HvigorCompilePluginPlacementResponse>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<HvigorCompilePluginDiagnosticResponse>,
}

pub(crate) struct HvigorCompilePluginNapiPlanner<'env> {
  planner: HvigorCompilePluginPlanner,
  descriptors: Vec<HvigorCompilePluginDescriptor>,
  callback_bridge: OrderCallbackBridge<'env>,
  input_error: Option<HvigorCompilePluginDiagnostic>,
}

impl<'env> HvigorCompilePluginNapiPlanner<'env> {
  pub(crate) fn from_js(env: Env, config_json: &str, plugins: Array<'env>) -> napi::Result<Self> {
    let config: HvigorCompilePluginPlanConfig = serde_json::from_str(config_json)
      .map_err(|error| napi::Error::from_reason(format!("parse compile plugin plan: {error}")))?;
    let mut descriptors = Vec::with_capacity(plugins.len() as usize);
    let mut callbacks = Vec::with_capacity(plugins.len() as usize);
    let mut input_error = None;

    for plugin_index in 0..plugins.len() {
      let Some(plugin) = plugins.get::<Object<'env>>(plugin_index)? else {
        return Err(napi::Error::from_reason(format!(
          "compile plugin[{plugin_index}] is missing"
        )));
      };
      let name = plugin.get::<String>("name")?.ok_or_else(|| {
        napi::Error::from_reason(format!("compile plugin[{plugin_index}] has no name"))
      })?;
      let (order, callback, diagnostic) = Self::read_order(&plugin, &name)?;
      if input_error.is_none() {
        input_error = diagnostic;
      }
      descriptors.push(HvigorCompilePluginDescriptor {
        source_index: plugin_index as usize,
        name,
        module_root: plugin.get::<String>("__arkdownModuleRoot")?,
        target_name: plugin.get::<String>("__arkdownTargetName")?,
        order,
      });
      callbacks.push(callback);
    }

    Ok(Self {
      planner: HvigorCompilePluginPlanner::new(config),
      descriptors,
      callback_bridge: OrderCallbackBridge::new(env, callbacks),
      input_error,
    })
  }

  pub(crate) fn plan_json(self) -> napi::Result<String> {
    if let Some(diagnostic) = self.input_error {
      return Self::serialize_response(Err(diagnostic));
    }

    let callback_bridge = self.callback_bridge;
    let result = self.planner.plan(self.descriptors, |plugin, names| {
      callback_bridge.invoke(plugin.source_index, names)
    });
    Self::serialize_response(result)
  }

  fn read_order(
    plugin: &Object<'env>,
    plugin_name: &str,
  ) -> napi::Result<(
    HvigorCompilePluginOrder,
    Option<OrderCallback<'env>>,
    Option<HvigorCompilePluginDiagnostic>,
  )> {
    let Some(value) = plugin.get::<Unknown<'env>>("__arkdownHvigorOrder")? else {
      return Ok((HvigorCompilePluginOrder::Last, None, None));
    };
    match value.get_type()? {
      ValueType::Undefined | ValueType::Null => Ok((HvigorCompilePluginOrder::Last, None, None)),
      ValueType::Function => Ok((
        HvigorCompilePluginOrder::Callback,
        Some(unsafe { value.cast::<OrderCallback<'env>>()? }),
        None,
      )),
      ValueType::String => {
        let order = unsafe { value.cast::<String>()? };
        match order.as_str() {
          "first" => Ok((HvigorCompilePluginOrder::First, None, None)),
          "last" => Ok((HvigorCompilePluginOrder::Last, None, None)),
          "" => Ok((HvigorCompilePluginOrder::Omit, None, None)),
          _ => Ok((
            HvigorCompilePluginOrder::Omit,
            None,
            Some(Self::unknown_order_diagnostic(
              plugin_name,
              &serde_json::to_string(&order).unwrap_or(order),
            )),
          )),
        }
      }
      ValueType::Boolean => {
        let value = unsafe { value.cast::<bool>()? };
        Ok(if value {
          (
            HvigorCompilePluginOrder::Omit,
            None,
            Some(Self::unknown_order_diagnostic(plugin_name, "true")),
          )
        } else {
          (HvigorCompilePluginOrder::Omit, None, None)
        })
      }
      ValueType::Number => {
        let value = unsafe { value.cast::<f64>()? };
        Ok(if value == 0.0 || value.is_nan() {
          (HvigorCompilePluginOrder::Omit, None, None)
        } else {
          (
            HvigorCompilePluginOrder::Omit,
            None,
            Some(Self::unknown_order_diagnostic(
              plugin_name,
              &value.to_string(),
            )),
          )
        })
      }
      // The binding targets N-API 4, where BigInt is reported as Unknown.
      // normalizeHvigorCompilePlugin already rejects truthy unknown orders;
      // the remaining observable case is falsy `0n`, which hvigor omits.
      ValueType::Unknown => Ok((HvigorCompilePluginOrder::Omit, None, None)),
      other => Ok((
        HvigorCompilePluginOrder::Omit,
        None,
        Some(Self::unknown_order_diagnostic(
          plugin_name,
          &other.to_string(),
        )),
      )),
    }
  }

  fn unknown_order_diagnostic(plugin_name: &str, value: &str) -> HvigorCompilePluginDiagnostic {
    HvigorCompilePluginDiagnostic {
      code: "PLUGIN_NAME_WITH_UNKNOWN_ORDER",
      message: format!("Plugin \"{plugin_name}\": unknown order value {value}"),
      plugin_name: Some(plugin_name.to_string()),
    }
  }

  fn serialize_response(
    result: std::result::Result<Vec<HvigorCompilePluginPlacement>, HvigorCompilePluginDiagnostic>,
  ) -> napi::Result<String> {
    let response = match result {
      Ok(placements) => HvigorCompilePluginPlanResponse {
        placements: placements
          .into_iter()
          .map(TryInto::try_into)
          .collect::<napi::Result<Vec<_>>>()?,
        error: None,
      },
      Err(diagnostic) => HvigorCompilePluginPlanResponse {
        placements: Vec::new(),
        error: Some(diagnostic.into()),
      },
    };
    serde_json::to_string(&response)
      .map_err(|error| napi::Error::from_reason(format!("serialize compile plugin plan: {error}")))
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RolldownPackagePluginPlacementResponse {
  plugin_index: u32,
  callback_resolved: bool,
}

impl TryFrom<RolldownPackagePluginPlacement> for RolldownPackagePluginPlacementResponse {
  type Error = napi::Error;

  fn try_from(placement: RolldownPackagePluginPlacement) -> napi::Result<Self> {
    Ok(Self {
      plugin_index: u32::try_from(placement.source_index)
        .map_err(|_| napi::Error::from_reason("package plugin index exceeds u32"))?,
      callback_resolved: placement.callback_resolved,
    })
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RolldownPackagePluginPlanResponse {
  placements: Vec<RolldownPackagePluginPlacementResponse>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  warnings: Vec<HvigorCompilePluginDiagnosticResponse>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<HvigorCompilePluginDiagnosticResponse>,
}

pub(crate) struct RolldownPackagePluginNapiPlanner<'env> {
  descriptors: Vec<RolldownPackagePluginDescriptor>,
  callback_bridge: OrderCallbackBridge<'env>,
  validation: HvigorPluginValidationOutcome,
}

impl<'env> RolldownPackagePluginNapiPlanner<'env> {
  pub(crate) fn from_js(env: Env, plugins: Array<'env>) -> napi::Result<Self> {
    let mut descriptors = Vec::with_capacity(plugins.len() as usize);
    let mut callbacks = Vec::with_capacity(plugins.len() as usize);
    let mut validation = HvigorPluginValidationOutcome::default();
    for plugin_index in 0..plugins.len() {
      let Some(plugin) = plugins.get::<Object<'env>>(plugin_index)? else {
        return Err(napi::Error::from_reason(format!(
          "package plugin[{plugin_index}] is missing"
        )));
      };
      let shape =
        HvigorPluginNapiShapeReader::read(plugin.to_unknown(), plugin_index as usize, None)?;
      let outcome = HvigorPluginShapeValidator::validate_package(&shape);
      validation.warnings.extend(outcome.warnings);
      if validation.error.is_none() {
        validation.error = outcome.error;
      }
      let name = shape.name.string_value.unwrap_or_default();
      let callback = match plugin.get::<Unknown<'env>>("order")? {
        Some(value) if value.get_type()? == ValueType::Function => {
          Some(unsafe { value.cast::<OrderCallback<'env>>()? })
        }
        _ => None,
      };
      descriptors.push(RolldownPackagePluginDescriptor {
        source_index: plugin_index as usize,
        name,
        has_order_callback: callback.is_some(),
      });
      callbacks.push(callback);
    }
    Ok(Self {
      descriptors,
      callback_bridge: OrderCallbackBridge::new(env, callbacks),
      validation,
    })
  }

  pub(crate) fn plan_json(self) -> napi::Result<String> {
    if let Some(diagnostic) = self.validation.error {
      return Self::serialize_response(Vec::new(), self.validation.warnings, Some(diagnostic));
    }
    let callback_bridge = self.callback_bridge;
    let result = RolldownPackagePluginPlanner::plan(self.descriptors, |plugin, names| {
      callback_bridge.invoke(plugin.source_index, names)
    });
    let (placements, error) = match result {
      Ok(placements) => (
        placements
          .into_iter()
          .map(TryInto::try_into)
          .collect::<napi::Result<Vec<_>>>()?,
        None,
      ),
      Err(diagnostic) => (Vec::new(), Some(diagnostic)),
    };
    Self::serialize_response(placements, self.validation.warnings, error)
  }

  fn serialize_response(
    placements: Vec<RolldownPackagePluginPlacementResponse>,
    warnings: Vec<HvigorCompilePluginDiagnostic>,
    error: Option<HvigorCompilePluginDiagnostic>,
  ) -> napi::Result<String> {
    let response = RolldownPackagePluginPlanResponse {
      placements,
      warnings: warnings.into_iter().map(Into::into).collect(),
      error: error.map(Into::into),
    };
    serde_json::to_string(&response)
      .map_err(|error| napi::Error::from_reason(format!("serialize package plugin plan: {error}")))
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorPluginValidationResponse {
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  warnings: Vec<HvigorCompilePluginDiagnosticResponse>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<HvigorCompilePluginDiagnosticResponse>,
}

pub(crate) struct HvigorPluginNapiValidator;

impl HvigorPluginNapiValidator {
  pub(crate) fn validate_compile(plugins: Array<'_>, source_file: String) -> napi::Result<String> {
    let mut validation = HvigorPluginValidationOutcome::default();
    for plugin_index in 0..plugins.len() {
      let plugin = plugins.get::<Unknown<'_>>(plugin_index)?.ok_or_else(|| {
        napi::Error::from_reason(format!("compile plugin[{plugin_index}] is missing"))
      })?;
      let shape = HvigorPluginNapiShapeReader::read(
        plugin,
        plugin_index as usize,
        Some(source_file.clone()),
      )?;
      let outcome = HvigorPluginShapeValidator::validate_compile(&shape);
      validation.warnings.extend(outcome.warnings);
      if outcome.error.is_some() {
        validation.error = outcome.error;
        break;
      }
    }
    serde_json::to_string(&HvigorPluginValidationResponse {
      warnings: validation.warnings.into_iter().map(Into::into).collect(),
      error: validation.error.map(Into::into),
    })
    .map_err(|error| napi::Error::from_reason(format!("serialize plugin validation: {error}")))
  }
}

struct HvigorPluginNapiShapeReader;

impl HvigorPluginNapiShapeReader {
  fn read(
    value: Unknown<'_>,
    source_index: usize,
    source_file: Option<String>,
  ) -> napi::Result<HvigorPluginShape> {
    let value_shape = Self::value_shape(value)?;
    if value_shape.kind != HvigorPluginValueKind::Object {
      return Ok(HvigorPluginShape {
        source_index,
        source_file,
        value: value_shape,
        name: Self::undefined_shape(),
        order: Self::undefined_shape(),
        attributes: Vec::new(),
      });
    }
    let object = unsafe { value.cast::<Object<'_>>()? };
    let name = Self::property_shape(&object, "name")?;
    let order = Self::property_shape(&object, "order")?;
    let attributes = Object::keys(&object)?
      .into_iter()
      .map(|name| {
        let value = Self::property_shape(&object, &name)?;
        Ok(HvigorPluginAttributeShape { name, value })
      })
      .collect::<napi::Result<Vec<_>>>()?;
    Ok(HvigorPluginShape {
      source_index,
      source_file,
      value: value_shape,
      name,
      order,
      attributes,
    })
  }

  fn property_shape(object: &Object<'_>, name: &str) -> napi::Result<HvigorPluginValueShape> {
    object
      .get::<Unknown<'_>>(name)?
      .map(Self::value_shape)
      .transpose()
      .map(|shape| shape.unwrap_or_else(Self::undefined_shape))
  }

  fn value_shape(value: Unknown<'_>) -> napi::Result<HvigorPluginValueShape> {
    let value_type = value.get_type()?;
    let truthy = value.coerce_to_bool()?;
    let (kind, string_value, description) = match value_type {
      ValueType::Undefined => (
        HvigorPluginValueKind::Undefined,
        None,
        "undefined".to_string(),
      ),
      ValueType::Null => (HvigorPluginValueKind::Null, None, "object".to_string()),
      ValueType::Boolean => {
        let value = unsafe { value.cast::<bool>()? };
        (HvigorPluginValueKind::Boolean, None, value.to_string())
      }
      ValueType::Number => {
        let value = unsafe { value.cast::<f64>()? };
        (HvigorPluginValueKind::Number, None, value.to_string())
      }
      ValueType::String => {
        let value = unsafe { value.cast::<String>()? };
        let description = serde_json::to_string(&value).unwrap_or_else(|_| value.clone());
        (HvigorPluginValueKind::String, Some(value), description)
      }
      ValueType::Function => (
        HvigorPluginValueKind::Function,
        None,
        "function".to_string(),
      ),
      ValueType::Object if value.is_array()? => {
        (HvigorPluginValueKind::Array, None, "array".to_string())
      }
      ValueType::Object => (HvigorPluginValueKind::Object, None, "object".to_string()),
      _ => (HvigorPluginValueKind::Other, None, value_type.to_string()),
    };
    Ok(HvigorPluginValueShape {
      kind,
      truthy,
      string_value,
      description,
    })
  }

  fn undefined_shape() -> HvigorPluginValueShape {
    HvigorPluginValueShape {
      kind: HvigorPluginValueKind::Undefined,
      truthy: false,
      string_value: None,
      description: "undefined".to_string(),
    }
  }
}
