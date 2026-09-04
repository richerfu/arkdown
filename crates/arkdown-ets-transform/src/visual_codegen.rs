//! Native Stage/ArkTS Visual model code generation.
//!
//! The SDK's `codegen/codegen_ets.js` deserializes the Visual v12 document,
//! applies a fixed component-parser table, then renders ArkUI source through
//! an indentation-aware writer. This module owns that behavior without a
//! JavaScript runtime. File discovery and source insertion remain outside the
//! generator so one parsed source snapshot can be shared by parallel builds.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde_json::{Map, Value};
use thiserror::Error;

use crate::VisualGeneratedSource;

const VISUAL_VERSION: u64 = 12;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VisualCodegenError {
    #[error("Visual file is damaged")]
    File,
    #[error("Version number of visual file does not match")]
    Version,
    #[error("Visual model in visual file is damaged")]
    Model,
    #[error("Codegen visual file failed: {0}")]
    Codegen(String),
}

/// Pure `.visual` document to ArkTS fragment compiler.
pub struct VisualCodeGenerator<'a> {
    source: &'a str,
}

impl<'a> VisualCodeGenerator<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub fn generate(&self) -> Result<VisualGeneratedSource, VisualCodegenError> {
        let document: Value =
            serde_json::from_str(self.source).map_err(|_| VisualCodegenError::File)?;
        let document = LegacyVisualConverter::new(document).normalize()?;
        let root = document.as_object().ok_or(VisualCodegenError::File)?;
        self.validate_version(root)?;

        let mut model =
            VisualModel::decode(root.get("visualModel").ok_or(VisualCodegenError::Model)?)?;
        let custom = root
            .get("document")
            .and_then(Value::as_object)
            .and_then(|document| document.get("custom"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if custom {
            let Some(child) = model.children.into_iter().next() else {
                return Ok(VisualGeneratedSource::default());
            };
            model = child;
        }

        let context = VisualCodegenContext::new(root, &model)?;
        let tag = VisualTagBuilder::new(&model, &context).build()?;
        Ok(VisualTagEmitter::new().emit(&tag))
    }

    fn validate_version(&self, root: &Map<String, Value>) -> Result<(), VisualCodegenError> {
        let version = root
            .get("document")
            .and_then(Value::as_object)
            .and_then(|document| document.get("VisualVersion"))
            .and_then(Value::as_str)
            .or_else(|| root.get("VisualVersion").and_then(Value::as_str))
            .ok_or(VisualCodegenError::Version)?;
        if version.is_empty()
            || !version.bytes().all(|byte| byte.is_ascii_digit())
            || version.starts_with('0')
            || version
                .parse::<u64>()
                .map_or(true, |version| version > VISUAL_VERSION)
        {
            return Err(VisualCodegenError::Version);
        }
        Ok(())
    }
}

/// Visual version 1 stored its tree as escaped JSON and could optionally
/// embed a complete v12 document in `contentV12`. Normalize it before the
/// regular typed-model decoder runs, matching the SDK deserializer.
struct LegacyVisualConverter {
    document: Value,
}

impl LegacyVisualConverter {
    fn new(document: Value) -> Self {
        Self { document }
    }

    fn normalize(self) -> Result<Value, VisualCodegenError> {
        let Some(root) = self.document.as_object() else {
            return Err(VisualCodegenError::File);
        };
        if root.get("VisualVersion").and_then(Value::as_str) != Some("1") {
            return Ok(self.document);
        }
        if let Some(content) = root.get("contentV12").and_then(Value::as_str) {
            return serde_json::from_str(content).map_err(|_| VisualCodegenError::File);
        }

        let content = root
            .get("content")
            .and_then(Value::as_str)
            .ok_or(VisualCodegenError::File)?;
        let content: Value = serde_json::from_str(content).map_err(|_| VisualCodegenError::File)?;
        let text_map = root
            .get("extraData")
            .and_then(Value::as_str)
            .and_then(|extra| serde_json::from_str::<Value>(extra).ok())
            .map_or_else(BTreeMap::new, |extra| Self::legacy_text_map(&extra));
        let mut model = Self::convert_node(&content, &text_map)?;
        if !model.properties.contains("flex-direction") {
            model.properties.0.push((
                "flex-direction".to_string(),
                Value::String("column".to_string()),
            ));
        }
        Ok(serde_json::json!({
            "document": {
                "VisualVersion": "12",
                "type": "FA",
                "custom": false,
                "path": ""
            },
            "visualModel": model.into_value()
        }))
    }

    fn convert_node(
        value: &Value,
        text_map: &BTreeMap<String, String>,
    ) -> Result<VisualModel, VisualCodegenError> {
        let object = value.as_object().ok_or(VisualCodegenError::Model)?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let tag_type = object
            .get("tagName")
            .and_then(Value::as_str)
            .ok_or(VisualCodegenError::Model)?
            .to_string();
        let mut properties = PropertyMap::default();
        for name in ["attributes", "idStyle"] {
            if let Some(entries) = object.get(name).and_then(Value::as_array) {
                for entry in entries {
                    let Some(pair) = entry.as_array() else {
                        continue;
                    };
                    let Some(key) = pair.first().and_then(Value::as_str) else {
                        continue;
                    };
                    if key == "id" {
                        continue;
                    }
                    let Some(value) = pair.get(1) else {
                        continue;
                    };
                    properties.set(key, value.clone());
                }
            }
        }
        let content = text_map
            .get(&id)
            .map(|value| Value::String(value.clone()))
            .or_else(|| {
                object
                    .get("content")
                    .filter(|value| value.is_string())
                    .cloned()
            });
        if let Some(content) = content {
            properties.set("content", content);
        }
        let children = object
            .get("content")
            .and_then(Value::as_array)
            .map(|children| {
                children
                    .iter()
                    .map(|child| Self::convert_node(child, text_map))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(VisualModel {
            id,
            tag_type,
            properties,
            dynamic_properties: PropertyMap::default(),
            media_properties: MediaPropertyMap::default(),
            dynamic_media_properties: MediaPropertyMap::default(),
            slots: Vec::new(),
            children,
        })
    }

    fn legacy_text_map(value: &Value) -> BTreeMap<String, String> {
        let Some(entries) = value.as_array() else {
            return BTreeMap::new();
        };
        let Some(text_map) = entries.iter().skip(1).find_map(|entry| {
            let pair = entry.as_array()?;
            (pair.first()?.as_str()? == "textMap")
                .then(|| pair.get(1))
                .flatten()
        }) else {
            return BTreeMap::new();
        };
        let Some(entries) = text_map.as_array() else {
            return BTreeMap::new();
        };
        entries
            .iter()
            .skip(1)
            .filter_map(|entry| {
                let pair = entry.as_array()?;
                Some((
                    pair.first()?.as_str()?.to_string(),
                    pair.get(1)?.as_str()?.to_string(),
                ))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct VisualModel {
    id: String,
    tag_type: String,
    properties: PropertyMap,
    dynamic_properties: PropertyMap,
    media_properties: MediaPropertyMap,
    dynamic_media_properties: MediaPropertyMap,
    slots: Vec<(String, Self)>,
    children: Vec<Self>,
}

impl VisualModel {
    fn decode(value: &Value) -> Result<Self, VisualCodegenError> {
        let value = Self::unwrap_typed(value, "VisualModel")?;
        let object = value.as_object().ok_or(VisualCodegenError::Model)?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut tag_type = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or(VisualCodegenError::Model)?
            .trim_end_matches("-visual")
            .to_string();
        if id == "wrapper" {
            tag_type = "div".to_string();
        }
        let properties = PropertyMap::decode(object.get("property"))?;
        let dynamic_properties = PropertyMap::decode(object.get("dynamicProperty"))?;
        let media_properties = MediaPropertyMap::decode(object.get("mediaProperty"))?;
        let dynamic_media_properties =
            MediaPropertyMap::decode(object.get("dynamicMediaProperty"))?;
        let slots = PropertyMap::decode(object.get("slots"))?
            .0
            .into_iter()
            .map(|(name, value)| Self::decode(&value).map(|model| (name, model)))
            .collect::<Result<Vec<_>, _>>()?;
        let children = object
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                children
                    .iter()
                    .map(Self::decode)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            id,
            tag_type,
            properties,
            dynamic_properties,
            media_properties,
            dynamic_media_properties,
            slots,
            children,
        })
    }

    fn unwrap_typed<'b>(value: &'b Value, expected: &str) -> Result<&'b Value, VisualCodegenError> {
        let Some(object) = value.as_object() else {
            return Err(VisualCodegenError::Model);
        };
        match object.get("dataType").and_then(Value::as_str) {
            Some(kind) if kind == expected => object.get("value").ok_or(VisualCodegenError::Model),
            Some(_) => Err(VisualCodegenError::Model),
            None => Ok(value),
        }
    }

    fn has(&self, name: &str) -> bool {
        self.properties.contains(name) || self.dynamic_properties.contains(name)
    }

    fn property(&self, name: &str) -> Option<&Value> {
        self.properties.get(name)
    }

    fn dynamic(&self, name: &str) -> Option<&Value> {
        self.dynamic_properties.get(name)
    }

    fn has_dynamic_value(&self, name: &str) -> bool {
        !VisualValue::is_empty(self.dynamic(name))
    }

    fn effective(&self, name: &str) -> Option<&Value> {
        self.dynamic(name)
            .filter(|value| !VisualValue::is_empty(Some(value)))
            .or_else(|| self.property(name))
    }

    fn has_media(&self, name: &str) -> bool {
        self.media_properties.contains(name) || self.dynamic_media_properties.contains(name)
    }

    fn into_value(self) -> Value {
        let property = self.properties.into_value();
        let dynamic_property = self.dynamic_properties.into_value();
        let media_property = self.media_properties.into_value();
        let dynamic_media_property = self.dynamic_media_properties.into_value();
        let children = self
            .children
            .into_iter()
            .map(Self::into_value)
            .collect::<Vec<_>>();
        serde_json::json!({
            "dataType": "VisualModel",
            "value": {
                "id": self.id,
                "type": self.tag_type,
                "property": property,
                "dynamicProperty": dynamic_property,
                "mediaProperty": media_property,
                "dynamicMediaProperty": dynamic_media_property,
                "children": children
            }
        })
    }
}

#[derive(Debug, Clone, Default)]
struct PropertyMap(Vec<(String, Value)>);

impl PropertyMap {
    fn decode(value: Option<&Value>) -> Result<Self, VisualCodegenError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let value = if let Some(object) = value.as_object() {
            match object.get("dataType").and_then(Value::as_str) {
                Some("Map") => object.get("value").ok_or(VisualCodegenError::Model)?,
                Some(_) => return Err(VisualCodegenError::Model),
                None => value,
            }
        } else {
            return Err(VisualCodegenError::Model);
        };
        let object = value.as_object().ok_or(VisualCodegenError::Model)?;
        Ok(Self(
            object
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
        ))
    }

    fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|(key, _)| key == name)
    }

    fn set(&mut self, name: impl Into<String>, value: Value) {
        let name = name.into();
        if let Some((_, current)) = self.0.iter_mut().find(|(key, _)| key == &name) {
            *current = value;
        } else {
            self.0.push((name, value));
        }
    }

    fn get(&self, name: &str) -> Option<&Value> {
        self.0
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value))
    }

    fn into_value(self) -> Value {
        Value::Object(Map::from_iter([
            ("dataType".to_string(), Value::String("Map".to_string())),
            ("value".to_string(), Value::Object(Map::from_iter(self.0))),
        ]))
    }
}

#[derive(Debug, Clone, Default)]
struct MediaPropertyMap(Vec<(String, PropertyMap)>);

impl MediaPropertyMap {
    fn decode(value: Option<&Value>) -> Result<Self, VisualCodegenError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let value = if let Some(object) = value.as_object() {
            match object.get("dataType").and_then(Value::as_str) {
                Some("Map") => object.get("value").ok_or(VisualCodegenError::Model)?,
                Some(_) => return Err(VisualCodegenError::Model),
                None => value,
            }
        } else {
            return Err(VisualCodegenError::Model);
        };
        let object = value.as_object().ok_or(VisualCodegenError::Model)?;
        Ok(Self(
            object
                .iter()
                .map(|(query, properties)| {
                    PropertyMap::decode(Some(properties))
                        .map(|properties| (query.clone(), properties))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn contains(&self, name: &str) -> bool {
        self.0
            .iter()
            .any(|(_, properties)| properties.contains(name))
    }

    fn get(&self, query: &str) -> Option<&PropertyMap> {
        self.0
            .iter()
            .find_map(|(current, properties)| (current == query).then_some(properties))
    }

    fn into_value(self) -> Value {
        Value::Object(Map::from_iter([
            ("dataType".to_string(), Value::String("Map".to_string())),
            (
                "value".to_string(),
                Value::Object(Map::from_iter(
                    self.0
                        .into_iter()
                        .map(|(query, properties)| (query, properties.into_value())),
                )),
            ),
        ]))
    }
}

struct VisualValue;

impl VisualValue {
    fn text(value: &Value) -> Option<String> {
        match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
            Value::Array(_) | Value::Object(_) => serde_json::to_string(value).ok(),
            Value::Null => None,
        }
    }

    fn optional_text(value: Option<&Value>) -> Option<String> {
        value.and_then(Self::text)
    }

    fn is_empty(value: Option<&Value>) -> bool {
        value.is_none_or(|value| matches!(value, Value::String(value) if value.is_empty()))
    }

    fn quote(value: &str) -> String {
        serde_json::to_string(value).expect("serialize Visual string")
    }

    fn identifier(name: &str) -> String {
        let name = name.split_once("--").map_or(name, |(name, _)| name);
        let mut words = name.split('-');
        let mut identifier = words.next().unwrap_or_default().to_string();
        for word in words {
            identifier.push_str(&Self::upper_first(word));
        }
        identifier
    }

    fn upper_first(value: &str) -> String {
        let mut characters = value.chars();
        let Some(first) = characters.next() else {
            return String::new();
        };
        first.to_uppercase().chain(characters).collect()
    }

    fn tag_name(name: &str) -> String {
        name.split('-').map(Self::upper_first).collect()
    }

    fn enum_value(prefix: &str, value: &str) -> String {
        format!("{prefix}{}", Self::tag_name(value))
    }

    fn color(value: &str) -> String {
        if value.starts_with('#') || value.starts_with("rgba") {
            Self::quote(value)
        } else if value.starts_with("0x") {
            value.to_string()
        } else {
            Self::enum_value("Color.", value)
        }
    }

    fn source(value: &str) -> String {
        if value.starts_with("$r('") || value.starts_with("$rawfile('") {
            value.to_string()
        } else {
            Self::quote(value)
        }
    }

    fn content_expression(value: &str) -> String {
        if value.starts_with("$r(") {
            value.to_string()
        } else {
            format!("`${{{value}}}`")
        }
    }

    fn font_weight(value: &str) -> String {
        if ["lighter", "normal", "regular", "medium", "bold", "bolder"].contains(&value) {
            Self::enum_value("FontWeight.", value)
        } else {
            value.to_string()
        }
    }

    fn unique_id(value: &str) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut bits = Vec::new();
        for unit in value.encode_utf16() {
            let width = (u16::BITS - unit.leading_zeros()).max(8) as usize;
            for shift in (0..width).rev() {
                bits.push(((unit >> shift) & 1) as u8);
            }
        }
        let remainder = bits.len() % 6;
        if remainder != 0 {
            bits.resize(bits.len() + 6 - remainder, 0);
        }
        let mut encoded = bits
            .chunks(6)
            .map(|chunk| {
                let value = chunk.iter().fold(0u8, |value, bit| (value << 1) | bit);
                ALPHABET[value as usize] as char
            })
            .filter(|character| !matches!(character, '/' | '=' | '+'))
            .collect::<String>();
        if encoded.len() >= 4 {
            let start = encoded.len() / 2 - 2;
            encoded = encoded[start..start + 4].to_string();
        }
        encoded
    }
}

#[derive(Debug, Clone)]
enum EtsValue {
    Text(String),
    Object(Vec<(String, EtsValue)>),
}

impl EtsValue {
    fn render(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Object(properties) => {
                let body = properties
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.render()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {body} }}")
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
enum TagParameters {
    #[default]
    Empty,
    Text(String),
    Object(Vec<(String, EtsValue)>),
    CustomObject(Vec<(String, EtsValue)>),
}

#[derive(Debug, Clone)]
struct ForEachModel {
    items: String,
    key: Option<String>,
    item: String,
    index: String,
}

#[derive(Debug, Clone)]
struct VisualTag {
    tag_name: String,
    parameters: TagParameters,
    properties: OrderedEtsProperties,
    children: Vec<Self>,
    for_each: Option<ForEachModel>,
    condition: Option<String>,
    defaults: Vec<VisualMediaDefault>,
    media_assignments: Vec<VisualMediaAssignment>,
    builders: Vec<(String, Self)>,
    custom_import: Option<(String, String)>,
}

#[derive(Debug, Clone)]
struct VisualMediaDefault {
    variable: String,
    value_type: String,
    initial: String,
    decorator: String,
}

#[derive(Debug, Clone)]
struct VisualMediaAssignment {
    query: String,
    variable: String,
    value: String,
}

#[derive(Debug, Clone, Default)]
struct OrderedEtsProperties(Vec<(String, EtsValue)>);

impl OrderedEtsProperties {
    fn set(&mut self, name: impl Into<String>, value: impl Into<EtsValue>) {
        let name = name.into();
        if let Some((_, current)) = self.0.iter_mut().find(|(key, _)| key == &name) {
            *current = value.into();
        } else {
            self.0.push((name, value.into()));
        }
    }
}

impl From<String> for EtsValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

#[derive(Debug)]
struct VisualCodegenContext {
    custom_components: Vec<CustomComponent>,
    custom_names: BTreeMap<String, String>,
    custom_imports: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct CustomComponent {
    path: String,
    attributes: Vec<CustomAttribute>,
    events: Vec<String>,
}

#[derive(Debug, Clone)]
struct CustomAttribute {
    source: String,
    target: String,
    value_type: String,
    decorator: String,
}

impl VisualCodegenContext {
    fn new(root: &Map<String, Value>, model: &VisualModel) -> Result<Self, VisualCodegenError> {
        let document_path = root
            .get("document")
            .and_then(Value::as_object)
            .and_then(|document| document.get("path"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut custom_components = Self::decode_custom_components(root.get("customData"))?;
        custom_components.sort_by(|left, right| left.path.cmp(&right.path));

        let mut paths = Vec::new();
        Self::collect_custom_paths(model, &mut paths);
        let mut leaves = BTreeMap::<String, Vec<String>>::new();
        for path in paths {
            let leaf = Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(VisualValue::upper_first)
                .unwrap_or_else(|| VisualValue::upper_first(&path));
            let values = leaves.entry(leaf).or_default();
            if !values.contains(&path) {
                values.push(path);
            }
        }
        let mut custom_names = BTreeMap::new();
        let mut custom_imports = BTreeMap::new();
        for (leaf, mut paths) in leaves {
            paths.sort();
            let document_leaf = document_path
                .rsplit('/')
                .next()
                .map(VisualValue::upper_first)
                .unwrap_or_default();
            let conflicts_with_document = leaf == document_leaf;
            let mut naming_paths = paths.clone();
            if conflicts_with_document {
                naming_paths.push(document_path.clone());
            }
            let common = Self::common_directory(&naming_paths);
            for path in paths.iter() {
                let name = if paths.len() > 1 || conflicts_with_document {
                    let relative = path
                        .strip_prefix(&common)
                        .unwrap_or(path)
                        .trim_start_matches('/');
                    VisualValue::upper_first(relative)
                        .replace('/', "_")
                        .replace("src_main_supervisual_", "")
                        .replace('-', "_")
                } else {
                    leaf.clone()
                };
                let import = Self::npm_import(path)
                    .unwrap_or_else(|| Self::relative_import(&document_path, path));
                custom_names.insert(path.clone(), name.clone());
                custom_imports.insert(path.clone(), import);
            }
        }
        Ok(Self {
            custom_components,
            custom_names,
            custom_imports,
        })
    }

    fn decode_custom_components(
        value: Option<&Value>,
    ) -> Result<Vec<CustomComponent>, VisualCodegenError> {
        let Some(value) = value.and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        let property_groups = PropertyMap::decode(value.get("property"))?;
        let event_groups = PropertyMap::decode(value.get("event"))?;
        let mut components = Vec::new();
        for (path, attributes) in property_groups.0 {
            let attributes = PropertyMap::decode(Some(&attributes))?
                .0
                .into_iter()
                .filter_map(|(name, descriptor)| {
                    let descriptor = descriptor.as_object()?;
                    let value_type = descriptor.get("type")?.as_str()?.to_string();
                    let decorator = descriptor
                        .get("decorator")
                        .and_then(Value::as_str)
                        .unwrap_or("normal")
                        .to_string();
                    let source = format!("{name}-{}", path.replace('/', "-"));
                    Some(CustomAttribute {
                        target: VisualValue::identifier(&source),
                        source,
                        value_type,
                        decorator,
                    })
                })
                .collect();
            let events = event_groups
                .get(&path)
                .and_then(Value::as_array)
                .map(|events| {
                    events
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            components.push(CustomComponent {
                path,
                attributes,
                events,
            });
        }
        for (path, events) in event_groups.0 {
            if components.iter().any(|component| component.path == path) {
                continue;
            }
            let events = events
                .as_array()
                .map(|events| {
                    events
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            components.push(CustomComponent {
                path,
                attributes: Vec::new(),
                events,
            });
        }
        Ok(components)
    }

    fn collect_custom_paths(model: &VisualModel, paths: &mut Vec<String>) {
        if model.tag_type.contains('/') {
            paths.push(model.tag_type.clone());
        }
        for child in &model.children {
            Self::collect_custom_paths(child, paths);
        }
    }

    fn common_directory(paths: &[String]) -> String {
        let Some(first) = paths.first() else {
            return String::new();
        };
        let mut end = first.len();
        for path in paths.iter().skip(1) {
            end = first
                .char_indices()
                .zip(path.chars())
                .take_while(|((offset, left), right)| *offset < end && left == right)
                .map(|((offset, character), _)| offset + character.len_utf8())
                .last()
                .unwrap_or_default();
        }
        first[..end]
            .rfind('/')
            .map_or(String::new(), |slash| first[..=slash].to_string())
    }

    fn relative_import(from: &str, to: &str) -> String {
        let from = Path::new(from)
            .components()
            .filter_map(Self::normal_component)
            .collect::<Vec<_>>();
        let to = Path::new(to)
            .components()
            .filter_map(Self::normal_component)
            .collect::<Vec<_>>();
        let common = from
            .iter()
            .zip(&to)
            .take_while(|(left, right)| left == right)
            .count();
        let mut relative = String::new();
        for _ in common..from.len().saturating_sub(1) {
            relative.push_str("../");
        }
        if relative.is_empty() {
            relative.push_str("./");
        }
        relative.push_str(&to[common..].join("/"));
        relative
    }

    fn normal_component(component: Component<'_>) -> Option<String> {
        match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::Prefix(value) => Some(value.as_os_str().to_string_lossy().into_owned()),
            _ => None,
        }
    }

    fn npm_import(path: &str) -> Option<String> {
        // Keep the SDK's `getNpmPath` matching and replacement behavior,
        // including its asymmetric `//node_modules` and `/oh_modules`
        // patterns. Visual import strings are observable generated output.
        let (marker, offset) = ["//node_modules/", "/oh_modules/"]
            .into_iter()
            .filter_map(|marker| path.find(marker).map(|offset| (marker, offset)))
            .min_by_key(|(_, offset)| *offset)?;
        let package_path = &path[offset + marker.len()..];
        let package_end = package_path.find("/src/main/")?;
        let package = &package_path[..package_end];
        let (scope, leaf) = package
            .rsplit_once('/')
            .map_or((None, package), |(scope, leaf)| (Some(scope), leaf));
        if leaf.is_empty()
            || !leaf.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        {
            return None;
        }
        let replacement = scope.map_or_else(String::new, |scope| format!("@{scope}/"));
        let matched_length = marker.len() + scope.map_or(0, |scope| scope.len() + 1);
        let mut npm_path = path.replacen("src/main/supervisual", "src/main/ets", 1);
        npm_path.replace_range(offset..offset + matched_length, &replacement);
        Some(npm_path)
    }

    fn is_custom(&self, path: &str) -> bool {
        self.custom_names.contains_key(path)
    }

    fn tag_name(&self, path: &str) -> String {
        self.custom_names
            .get(path)
            .cloned()
            .unwrap_or_else(|| VisualValue::tag_name(path.rsplit('/').next().unwrap_or(path)))
    }

    fn import(&self, path: &str) -> Option<(String, String)> {
        Some((
            self.custom_names.get(path)?.clone(),
            self.custom_imports.get(path)?.clone(),
        ))
    }

    fn component(&self, path: &str) -> Option<&CustomComponent> {
        self.custom_components
            .iter()
            .find(|component| component.path == path)
    }

    fn builder_name(&self, id: &str, slot: &str) -> String {
        let id = self.custom_names.get(id).map_or(id, String::as_str);
        let base = format!(
            "{}{}",
            VisualValue::identifier(id),
            VisualValue::upper_first(&VisualValue::identifier(slot))
        );
        format!("{base}_{}", VisualValue::unique_id(&base))
    }
}

struct VisualTagBuilder<'a> {
    model: &'a VisualModel,
    context: &'a VisualCodegenContext,
    parameters: TagParameters,
    properties: OrderedEtsProperties,
    defaults: Vec<VisualMediaDefault>,
    media_assignments: Vec<VisualMediaAssignment>,
    media_for_each: Option<ForEachModel>,
    media_condition: Option<String>,
}

impl<'a> VisualTagBuilder<'a> {
    fn new(model: &'a VisualModel, context: &'a VisualCodegenContext) -> Self {
        Self {
            model,
            context,
            parameters: TagParameters::Empty,
            properties: OrderedEtsProperties::default(),
            defaults: Vec::new(),
            media_assignments: Vec::new(),
            media_for_each: None,
            media_condition: None,
        }
    }

    fn build(mut self) -> Result<VisualTag, VisualCodegenError> {
        self.parse_common();
        self.parse_component()?;
        self.parse_media();
        self.parse_custom();
        self.parse_events()?;
        let children = self
            .model
            .children
            .iter()
            .map(|child| Self::new(child, self.context).build())
            .collect::<Result<Vec<_>, _>>()?;
        let builders = self
            .model
            .slots
            .iter()
            .map(|(slot, model)| {
                let name = self.context.builder_name(&self.model.id, slot);
                Self::new(model, self.context)
                    .build()
                    .map(|tag| (name, tag))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (slot, _) in &self.model.slots {
            let name = self.context.builder_name(&self.model.id, slot);
            self.properties.set(slot, format!("this.{name}"));
        }
        let for_each = self.media_for_each.clone().or_else(|| self.for_each());
        let condition = self.media_condition.clone().or_else(|| {
            VisualValue::optional_text(
                self.model
                    .dynamic("if")
                    .or_else(|| self.model.property("if")),
            )
        });
        Ok(VisualTag {
            tag_name: self.context.tag_name(&self.model.tag_type),
            parameters: if self.context.is_custom(&self.model.tag_type) {
                match self.parameters {
                    TagParameters::Object(values) => TagParameters::CustomObject(values),
                    parameters => parameters,
                }
            } else {
                self.parameters
            },
            properties: self.properties,
            children,
            for_each,
            condition,
            defaults: self.defaults,
            media_assignments: self.media_assignments,
            builders,
            custom_import: self.context.import(&self.model.tag_type),
        })
    }

    fn for_each(&self) -> Option<ForEachModel> {
        let items = VisualValue::optional_text(self.model.dynamic("for"))?;
        Some(ForEachModel {
            items,
            key: VisualValue::optional_text(self.model.dynamic("keyGenerator")),
            item: VisualValue::optional_text(self.model.property("item"))
                .unwrap_or_else(|| "item".to_string()),
            index: VisualValue::optional_text(self.model.property("idx"))
                .unwrap_or_else(|| "idx".to_string()),
        })
    }

    fn set_parameter(&mut self, name: &str, value: String) {
        match &mut self.parameters {
            TagParameters::Empty => {
                self.parameters = TagParameters::Object(vec![(name.to_string(), value.into())]);
            }
            TagParameters::Object(parameters) | TagParameters::CustomObject(parameters) => {
                if let Some((_, current)) = parameters.iter_mut().find(|(key, _)| key == name) {
                    *current = value.into();
                } else {
                    parameters.push((name.to_string(), value.into()));
                }
            }
            TagParameters::Text(_) => {}
        }
    }

    fn set_text_parameters(&mut self, value: String) {
        self.parameters = TagParameters::Text(value);
    }

    fn parse_common(&mut self) {
        self.string_property("width", "width");
        self.string_property("height", "height");
        self.constraint_size();
        self.enum_property("align", "align", "Alignment.");
        self.enum_property("direction", "direction", "Direction.");
        self.position();
        self.raw_property("aspect-ratio", "aspectRatio");
        self.raw_property("display-priority", "displayPriority");
        self.string_property("flex-basis", "flexBasis");
        self.raw_property("flex-grow", "flexGrow");
        self.raw_property("flex-shrink", "flexShrink");
        self.align_self();
        self.border_style();
        self.border_edges("border-width", VisualValue::quote);
        self.border_edges("border-color", VisualValue::color);
        self.border_edges("border-radius", VisualValue::quote);
        self.color_property("background-color", "backgroundColor");
        self.background_image();
        self.background_image_size();
        self.background_image_position();
        self.raw_property("opacity", "opacity");
        self.enum_property("visibility", "visibility", "Visibility.");
        self.raw_property("enabled", "enabled");
        self.color_property("font-color", "fontColor");
        self.box_edges("margin");
        self.box_edges("padding");
    }

    fn constraint_size(&mut self) {
        let mut properties = Vec::new();
        for (source, target) in [
            ("constraint-size-min-width", "minWidth"),
            ("constraint-size-max-width", "maxWidth"),
            ("constraint-size-min-height", "minHeight"),
            ("constraint-size-max-height", "maxHeight"),
        ] {
            let dynamic = self.model.dynamic(source);
            let value = dynamic.or_else(|| self.model.property(source));
            let Some(value) = VisualValue::optional_text(value) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            properties.push((
                target.to_string(),
                if dynamic.is_some() {
                    value
                } else {
                    VisualValue::quote(&value)
                }
                .into(),
            ));
        }
        if !properties.is_empty() {
            self.properties
                .set("constraintSize", EtsValue::Object(properties));
        }
    }

    fn position(&mut self) {
        if !self.model.has("left") && !self.model.has("top") {
            return;
        }
        let property = if VisualValue::optional_text(self.model.property("position")).as_deref()
            == Some("absolute")
        {
            "position"
        } else {
            "offset"
        };
        let mut values = Vec::new();
        for (source, target) in [("left", "x"), ("top", "y")] {
            let dynamic = self.model.dynamic(source);
            let value = VisualValue::optional_text(dynamic.or_else(|| self.model.property(source)))
                .unwrap_or_else(|| "0".to_string());
            values.push((
                target.to_string(),
                if dynamic.is_some() {
                    value
                } else {
                    VisualValue::quote(&value)
                }
                .into(),
            ));
        }
        self.properties.set(property, EtsValue::Object(values));
    }

    fn align_self(&mut self) {
        let Some(value) = VisualValue::optional_text(self.model.effective("align-self")) else {
            return;
        };
        let value = if self.model.dynamic("align-self").is_some() {
            value
        } else if value == "flex-start" {
            "ItemAlign.Start".to_string()
        } else if value == "flex-end" {
            "ItemAlign.End".to_string()
        } else {
            VisualValue::enum_value("ItemAlign.", &value)
        };
        self.properties.set("alignSelf", value);
    }

    fn border_style(&mut self) {
        let source = "border-style";
        let edges = [
            ("border-top-style", "top"),
            ("border-bottom-style", "bottom"),
            ("border-left-style", "left"),
            ("border-right-style", "right"),
        ];
        if !self.model.has(source) && !edges.iter().any(|(edge, _)| self.model.has(edge)) {
            return;
        }
        let has_edge = edges
            .iter()
            .any(|(edge, _)| !VisualValue::is_empty(self.model.effective(edge)));
        if !has_edge {
            if let Some(value) = VisualValue::optional_text(self.model.effective(source)) {
                let value = if self.model.dynamic(source).is_some() {
                    value
                } else {
                    VisualValue::enum_value("BorderStyle.", &value)
                };
                self.properties.set("borderStyle", value);
            }
            return;
        }
        let mut values = Vec::new();
        for (edge, target) in edges {
            let dynamic = self.model.dynamic(edge).or_else(|| {
                (!self.model.properties.contains(edge))
                    .then(|| self.model.dynamic(source))
                    .flatten()
            });
            let value = self
                .model
                .effective(edge)
                .or_else(|| self.model.effective(source));
            if let Some(value) = VisualValue::optional_text(value).filter(|value| !value.is_empty())
            {
                values.push((
                    target.to_string(),
                    if dynamic.is_some() {
                        value
                    } else {
                        VisualValue::enum_value("BorderStyle.", &value)
                    }
                    .into(),
                ));
            }
        }
        if !values.is_empty() {
            self.properties.set("borderStyle", EtsValue::Object(values));
        }
    }

    fn border_edges(&mut self, source: &str, convert: fn(&str) -> String) {
        let suffix = source.trim_start_matches("border-");
        let edges = if suffix == "radius" {
            vec![
                ("border-top-left-radius".to_string(), "topLeft"),
                ("border-bottom-right-radius".to_string(), "bottomRight"),
                ("border-bottom-left-radius".to_string(), "bottomLeft"),
                ("border-top-right-radius".to_string(), "topRight"),
            ]
        } else {
            vec![
                (format!("border-top-{suffix}"), "top"),
                (format!("border-bottom-{suffix}"), "bottom"),
                (format!("border-left-{suffix}"), "left"),
                (format!("border-right-{suffix}"), "right"),
            ]
        };
        if !self.model.has(source) && !edges.iter().any(|(edge, _)| self.model.has(edge)) {
            return;
        }
        let has_edge = edges
            .iter()
            .any(|(edge, _)| !VisualValue::is_empty(self.model.effective(edge)));
        let target = VisualValue::identifier(source);
        if !has_edge {
            if let Some(value) = VisualValue::optional_text(self.model.effective(source)) {
                let value = if self.model.dynamic(source).is_some() {
                    value
                } else {
                    convert(&value)
                };
                self.properties.set(target, value);
            }
            return;
        }
        let mut values = Vec::new();
        for (edge, target) in edges {
            let dynamic = self.model.dynamic(&edge).or_else(|| {
                (!self.model.properties.contains(&edge))
                    .then(|| self.model.dynamic(source))
                    .flatten()
            });
            let value = self
                .model
                .effective(&edge)
                .or_else(|| self.model.effective(source));
            if let Some(value) = VisualValue::optional_text(value).filter(|value| !value.is_empty())
            {
                values.push((
                    target.to_string(),
                    if dynamic.is_some() {
                        value
                    } else {
                        convert(&value)
                    }
                    .into(),
                ));
            }
        }
        if !values.is_empty() {
            self.properties.set(target, EtsValue::Object(values));
        }
    }

    fn background_image(&mut self) {
        if !self.model.has("background-image-src") && !self.model.has("background-image-repeat") {
            return;
        }
        let mut value = self
            .model
            .dynamic("background-image-src")
            .filter(|value| !VisualValue::is_empty(Some(value)))
            .and_then(VisualValue::text)
            .or_else(|| {
                VisualValue::optional_text(self.model.property("background-image-src"))
                    .map(|value| VisualValue::source(&value))
            })
            .unwrap_or_else(|| VisualValue::quote(""));
        if let Some(repeat) = VisualValue::optional_text(
            self.model
                .dynamic("background-image-repeat")
                .filter(|value| !VisualValue::is_empty(Some(value)))
                .or_else(|| self.model.property("background-image-repeat")),
        ) {
            let repeat = if self.model.has_dynamic_value("background-image-repeat") {
                repeat
            } else {
                match repeat.as_str() {
                    "repeat-x" => "ImageRepeat.X".to_string(),
                    "repeat-y" => "ImageRepeat.Y".to_string(),
                    "repeat" => "ImageRepeat.XY".to_string(),
                    "no-repeat" => "ImageRepeat.NoRepeat".to_string(),
                    _ => String::new(),
                }
            };
            if !repeat.is_empty() {
                value.push_str(", ");
                value.push_str(&repeat);
            }
        }
        self.properties.set("backgroundImage", value);
    }

    fn background_image_size(&mut self) {
        let Some(value) = VisualValue::optional_text(self.model.effective("background-image-size"))
        else {
            return;
        };
        if self.model.has_dynamic_value("background-image-size") {
            self.properties.set("backgroundImageSize", value);
            return;
        }
        if ["cover", "contain", "auto"].contains(&value.as_str()) {
            self.properties.set(
                "backgroundImageSize",
                VisualValue::enum_value("ImageSize.", &value),
            );
            return;
        }
        let values = value.split_ascii_whitespace().collect::<Vec<_>>();
        if values.is_empty()
            || values.len() > 2
            || values.iter().any(|value| !Self::is_dimension(value))
        {
            return;
        }
        let mut properties = vec![("width".to_string(), VisualValue::quote(values[0]).into())];
        if let Some(height) = values.get(1) {
            properties.push(("height".to_string(), VisualValue::quote(height).into()));
        }
        self.properties
            .set("backgroundImageSize", EtsValue::Object(properties));
    }

    fn background_image_position(&mut self) {
        let Some(value) =
            VisualValue::optional_text(self.model.effective("background-image-position"))
        else {
            return;
        };
        if self.model.has_dynamic_value("background-image-position") {
            self.properties.set("backgroundImagePosition", value);
            return;
        }
        let alignment = match value.as_str() {
            "top left" => Some("TopStart"),
            "top center" => Some("Top"),
            "top right" => Some("TopEnd"),
            "center left" => Some("Start"),
            "center center" => Some("Center"),
            "center right" => Some("End"),
            "bottom left" => Some("BottomStart"),
            "bottom center" => Some("Bottom"),
            "bottom right" => Some("BottomEnd"),
            _ => None,
        };
        if let Some(alignment) = alignment {
            self.properties
                .set("backgroundImagePosition", format!("Alignment.{alignment}"));
            return;
        }
        let values = value.split_ascii_whitespace().collect::<Vec<_>>();
        if values.len() == 2 && values.iter().all(|value| Self::is_dimension(value)) {
            self.properties.set(
                "backgroundImagePosition",
                EtsValue::Object(vec![
                    ("x".to_string(), VisualValue::quote(values[0]).into()),
                    ("y".to_string(), VisualValue::quote(values[1]).into()),
                ]),
            );
        }
    }

    fn is_dimension(value: &str) -> bool {
        let suffix = ["px", "vp", "lpx", "%"]
            .into_iter()
            .find(|suffix| value.ends_with(suffix));
        suffix.is_some_and(|suffix| {
            let number = &value[..value.len() - suffix.len()];
            number == "0"
                || (!number.starts_with('0') && number.bytes().all(|byte| byte.is_ascii_digit()))
        })
    }

    fn background_image_size_enum(value: &str) -> String {
        match value.trim() {
            "contain" => "ImageSize.Contain".to_string(),
            "cover" => "ImageSize.Cover".to_string(),
            "auto" => "ImageSize.Auto".to_string(),
            _ => String::new(),
        }
    }

    fn background_image_size_object(value: &str) -> String {
        let values = value.split_whitespace().collect::<Vec<_>>();
        if values.is_empty()
            || values.len() > 2
            || !values.iter().all(|value| Self::is_dimension(value))
        {
            return "undefined".to_string();
        }
        if values.len() == 2 {
            format!(
                "{{ width: {}, height: {} }}",
                VisualValue::quote(values[0]),
                VisualValue::quote(values[1])
            )
        } else {
            format!("{{ width: {} }}", VisualValue::quote(values[0]))
        }
    }

    fn background_image_position_enum(value: &str) -> String {
        let alignment = match value.trim() {
            "top left" => Some("TopStart"),
            "top center" => Some("Top"),
            "top right" => Some("TopEnd"),
            "center left" => Some("Start"),
            "center center" => Some("Center"),
            "center right" => Some("End"),
            "bottom left" => Some("BottomStart"),
            "bottom center" => Some("Bottom"),
            "bottom right" => Some("BottomEnd"),
            _ => None,
        };
        alignment.map_or_else(String::new, |value| format!("Alignment.{value}"))
    }

    fn background_image_position_object(value: &str) -> String {
        let values = value.split_whitespace().collect::<Vec<_>>();
        if values.len() != 2 || !values.iter().all(|value| Self::is_dimension(value)) {
            return "undefined".to_string();
        }
        format!(
            "{{ x: {}, y: {} }}",
            VisualValue::quote(values[0]),
            VisualValue::quote(values[1])
        )
    }

    fn box_edges(&mut self, name: &str) {
        let edges = ["top", "bottom", "left", "right"];
        if !self.model.has(name)
            && !edges
                .iter()
                .any(|edge| self.model.has(&format!("{name}-{edge}")))
        {
            return;
        }
        let mut values = Vec::new();
        for edge in edges {
            let edge_name = format!("{name}-{edge}");
            let has_static_edge = self.model.properties.contains(&edge_name);
            let dynamic = self.model.dynamic(&edge_name).or_else(|| {
                (!has_static_edge)
                    .then(|| self.model.dynamic(name))
                    .flatten()
            });
            let value = self
                .model
                .effective(&edge_name)
                .or_else(|| self.model.effective(name));
            if let Some(value) = VisualValue::optional_text(value).filter(|value| !value.is_empty())
            {
                values.push((
                    edge.to_string(),
                    if dynamic.is_some() {
                        value
                    } else {
                        VisualValue::quote(&value)
                    }
                    .into(),
                ));
            }
        }
        if !values.is_empty() {
            self.properties.set(name, EtsValue::Object(values));
        }
    }

    fn parse_component(&mut self) -> Result<(), VisualCodegenError> {
        match self.model.tag_type.as_str() {
            "button" => self.parse_button(),
            "divider" => self.parse_divider(),
            "image" => self.parse_image(),
            "progress" => self.parse_progress(),
            "slider" => self.parse_slider(),
            "text" => self.parse_text(),
            "column" => self.parse_column(),
            "row" => self.parse_row(),
            "flex" => self.parse_flex(),
            "list" => self.parse_list(),
            "list-item" => self.parse_list_item(),
            "swiper" => self.parse_swiper(),
            "text-input" => self.parse_text_input(),
            "navigation" => self.parse_navigation(),
            "timePicker" => self.parse_time_picker(),
            "textPicker" => self.parse_text_picker(),
            "refresh" => self.parse_refresh(),
            "toggle" => self.parse_toggle(),
            "select" => self.parse_select(),
            "datePicker" => self.parse_date_picker(),
            "scroll" => self.parse_scroll(),
            "grid" => self.parse_grid(),
            "grid-item" => self.parse_grid_item(),
            "search" => self.parse_search(),
            "stack" => self.parse_stack(),
            "tabs" => self.parse_tabs(),
            "tab-content" => self.parse_tab_content(),
            "checkbox" => self.parse_checkbox(),
            "checkboxGroup" => self.parse_checkbox_group(),
            _ => {}
        }
        Ok(())
    }

    fn parse_button(&mut self) {
        if self.model.has("label") {
            if let Some(value) = VisualValue::optional_text(self.model.dynamic("label"))
                .filter(|value| !value.is_empty())
            {
                self.set_text_parameters(VisualValue::content_expression(&value));
            } else if let Some(value) = VisualValue::optional_text(self.model.property("label")) {
                self.set_text_parameters(VisualValue::quote(&value));
            }
        }
        self.enum_property("type", "type", "ButtonType.");
        self.raw_property("state-effect", "stateEffect");
        self.string_property("font-size", "fontSize");
        self.enum_property("font-style", "fontStyle", "FontStyle.");
        self.font_weight_property("font-weight", "fontWeight");
        self.string_property("font-family", "fontFamily");
    }

    fn parse_divider(&mut self) {
        self.raw_property("vertical", "vertical");
        self.color_property("color", "color");
        self.string_property("stroke-width", "strokeWidth");
        self.enum_property("line-cap", "lineCap", "LineCapStyle.");
    }

    fn parse_image(&mut self) {
        let src = self
            .model
            .dynamic("src")
            .filter(|value| !VisualValue::is_empty(Some(value)))
            .and_then(VisualValue::text)
            .or_else(|| {
                VisualValue::optional_text(self.model.property("src"))
                    .map(|value| VisualValue::source(&value))
            })
            .unwrap_or_else(|| VisualValue::quote(""));
        self.set_text_parameters(src);
        self.string_property("alt", "alt");
        self.enum_property("object-fit", "objectFit", "ImageFit.");
        self.enum_property("object-repeat", "objectRepeat", "ImageRepeat.");
        self.enum_property("interpolation", "interpolation", "ImageInterpolation.");
        self.enum_property("render-mode", "renderMode", "ImageRenderMode.");
        if self.model.has("source-size-width") || self.model.has("source-size-height") {
            let mut values = Vec::new();
            for (source, target) in [
                ("source-size-width", "width"),
                ("source-size-height", "height"),
            ] {
                let dynamic = self.model.dynamic(source).is_some();
                let mut value = VisualValue::optional_text(self.model.effective(source))
                    .unwrap_or_else(|| "0".to_string());
                if !dynamic && value != "0" {
                    value.truncate(value.len().saturating_sub(2));
                }
                values.push((target.to_string(), value.into()));
            }
            self.properties.set("sourceSize", EtsValue::Object(values));
        }
    }

    fn parse_progress(&mut self) {
        if self.model.has("value") || self.model.has("total") || self.model.has("style") {
            let value = VisualValue::optional_text(self.model.effective("value"))
                .unwrap_or_else(|| "0".to_string());
            self.set_parameter("value", value);
        }
        self.raw_parameter("total", "total");
        self.enum_parameter("style", "style", "ProgressStyle.");
        self.color_property("color", "color");
    }

    fn parse_slider(&mut self) {
        for property in ["value", "min", "max", "step"] {
            self.raw_parameter(property, property);
        }
        self.enum_parameter("style", "style", "SliderStyle.");
        self.color_property("block-color", "blockColor");
        self.color_property("track-color", "trackColor");
        self.color_property("selected-color", "selectedColor");
        self.raw_property("show-steps", "showSteps");
        self.raw_property("show-tips", "showTips");
    }

    fn parse_text(&mut self) {
        if let Some(value) = VisualValue::optional_text(self.model.dynamic("content"))
            .filter(|value| !value.is_empty())
        {
            self.set_text_parameters(VisualValue::content_expression(&value));
        } else if let Some(value) = VisualValue::optional_text(self.model.property("content")) {
            self.set_text_parameters(VisualValue::quote(&value));
        }
        self.enum_property("text-align", "textAlign", "TextAlign.");
        if self.model.has("text-overflow") {
            if let Some(value) = VisualValue::optional_text(self.model.effective("text-overflow")) {
                let value = if self.model.dynamic("text-overflow").is_some() {
                    value
                } else {
                    VisualValue::enum_value("TextOverflow.", &value)
                };
                self.properties.set(
                    "textOverflow",
                    EtsValue::Object(vec![("overflow".to_string(), value.into())]),
                );
            }
        }
        self.raw_property("max-lines", "maxLines");
        self.string_property("line-height", "lineHeight");
        self.text_decoration();
        self.string_property("baseline-offset", "baselineOffset");
        self.enum_property("text-case", "textCase", "TextCase.");
        self.string_property("font-size", "fontSize");
        self.enum_property("font-style", "fontStyle", "FontStyle.");
        self.font_weight_property("font-weight", "fontWeight");
        self.string_property("font-family", "fontFamily");
    }

    fn text_decoration(&mut self) {
        if !self.model.has("decoration-type") && !self.model.has("decoration-color") {
            return;
        }
        let mut values = Vec::new();
        let type_value = VisualValue::optional_text(self.model.effective("decoration-type"));
        values.push((
            "type".to_string(),
            if self.model.dynamic("decoration-type").is_some() {
                type_value.unwrap_or_default()
            } else {
                type_value.map_or_else(
                    || "TextDecorationType.None".to_string(),
                    |value| VisualValue::enum_value("TextDecorationType.", &value),
                )
            }
            .into(),
        ));
        if let Some(color) = VisualValue::optional_text(self.model.effective("decoration-color")) {
            values.push((
                "color".to_string(),
                if self.model.dynamic("decoration-color").is_some() {
                    color
                } else {
                    VisualValue::color(&color)
                }
                .into(),
            ));
        }
        self.properties.set("decoration", EtsValue::Object(values));
    }

    fn parse_column(&mut self) {
        self.string_parameter("space", "space");
        self.column_row_alignment("align-items-column", "alignItems", "HorizontalAlign.");
        self.justify_content("justify-content-rc", "justifyContent");
    }

    fn parse_row(&mut self) {
        self.string_parameter("space", "space");
        self.column_row_alignment("align-items-row", "alignItems", "VerticalAlign.");
        self.justify_content("justify-content-rc", "justifyContent");
    }

    fn column_row_alignment(&mut self, source: &str, target: &str, prefix: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        let value = if self.model.has_dynamic_value(source) {
            value
        } else if prefix == "HorizontalAlign." && value == "flex-start" {
            "HorizontalAlign.Start".to_string()
        } else if prefix == "HorizontalAlign." && value == "flex-end" {
            "HorizontalAlign.End".to_string()
        } else if prefix == "VerticalAlign." && value == "flex-start" {
            "VerticalAlign.Top".to_string()
        } else if prefix == "VerticalAlign." && value == "flex-end" {
            "VerticalAlign.Bottom".to_string()
        } else {
            VisualValue::enum_value(prefix, &value)
        };
        self.properties.set(target, value);
    }

    fn justify_content(&mut self, source: &str, target: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        let value = if self.model.has_dynamic_value(source) {
            value
        } else if value == "flex-start" {
            "FlexAlign.Start".to_string()
        } else if value == "flex-end" {
            "FlexAlign.End".to_string()
        } else {
            VisualValue::enum_value("FlexAlign.", &value)
        };
        self.properties.set(target, value);
    }

    fn parse_flex(&mut self) {
        self.enum_parameter("flex-direction", "direction", "FlexDirection.");
        if let Some(value) = VisualValue::optional_text(self.model.effective("wrap")) {
            let value = if self.model.has_dynamic_value("wrap") {
                value
            } else if value == "nowrap" {
                "FlexWrap.NoWrap".to_string()
            } else {
                VisualValue::enum_value("FlexWrap.", &value)
            };
            self.set_parameter("wrap", value);
        }
        self.flex_parameter("justify-content", "justifyContent", "FlexAlign.");
        self.flex_parameter("align-items-flex", "alignItems", "ItemAlign.");
        self.flex_parameter("align-content", "alignContent", "FlexAlign.");
    }

    fn flex_parameter(&mut self, source: &str, target: &str, prefix: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        let value = if self.model.has_dynamic_value(source) {
            value
        } else if value == "flex-start" {
            format!("{prefix}Start")
        } else if value == "flex-end" {
            format!("{prefix}End")
        } else {
            VisualValue::enum_value(prefix, &value)
        };
        self.set_parameter(target, value);
    }

    fn parse_list(&mut self) {
        self.string_parameter("space", "space");
        self.raw_parameter("initial-index", "initialIndex");
        self.enum_property("list-direction", "listDirection", "Axis.");
        self.list_divider();
        self.raw_property("edit-mode", "editMode");
        self.enum_property("edge-effect", "edgeEffect", "EdgeEffect.");
        self.raw_property("chain-animation", "chainAnimation");
    }

    fn list_divider(&mut self) {
        let sources = [
            ("divider-stroke-width", "strokeWidth"),
            ("divider-color", "color"),
            ("divider-start-margin", "startMargin"),
            ("divider-end-margin", "endMargin"),
        ];
        if !sources.iter().any(|(source, _)| self.model.has(source)) {
            return;
        }
        let mut values = vec![(
            "strokeWidth".to_string(),
            VisualValue::quote(
                &VisualValue::optional_text(self.model.property("divider-stroke-width"))
                    .unwrap_or_else(|| "0".to_string()),
            )
            .into(),
        )];
        if let Some(dynamic) =
            VisualValue::optional_text(self.model.dynamic("divider-stroke-width"))
                .filter(|value| !value.is_empty())
        {
            values[0].1 = dynamic.into();
        }
        for (source, target) in sources.into_iter().skip(1) {
            let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
                continue;
            };
            let value = if self.model.has_dynamic_value(source) {
                value
            } else if source == "divider-color" {
                VisualValue::color(&value)
            } else {
                VisualValue::quote(&value)
            };
            values.push((target.to_string(), value.into()));
        }
        self.properties.set("divider", EtsValue::Object(values));
    }

    fn parse_list_item(&mut self) {
        self.enum_property("sticky", "sticky", "Sticky.");
        self.raw_property("editable", "editable");
    }

    fn parse_swiper(&mut self) {
        self.raw_property("index", "index");
        self.raw_property("auto-play", "autoPlay");
        self.raw_property("interval", "interval");
        self.raw_property("indicator", "indicator");
        self.raw_property("loop", "loop");
        self.raw_property("duration", "duration");
        self.raw_property("vertical", "vertical");
        self.string_property("item-space", "itemSpace");
        self.raw_property("cached-count", "cachedCount");
        self.raw_property("disable-swipe", "disableSwipe");
    }

    fn parse_text_input(&mut self) {
        self.string_parameter("placeholder", "placeholder");
        if let Some(value) = VisualValue::optional_text(self.model.effective("textInput-type")) {
            let value = if self.model.has_dynamic_value("textInput-type") {
                value
            } else {
                format!("InputType.{value}")
            };
            self.properties.set("type", value);
        }
        self.color_property("placeholderColor", "placeholderColor");
        self.text_input_placeholder_font();
        self.enum_property("enterKeyType", "enterKeyType", "EnterKeyType.");
        self.color_property("caretColor", "caretColor");
        self.raw_property("maxLength", "maxLength");
    }

    fn text_input_placeholder_font(&mut self) {
        let mut values = Vec::new();
        for (suffix, target) in [
            ("size", "size"),
            ("weight", "weight"),
            ("family", "family"),
            ("style", "style"),
        ] {
            let source = format!("placeholderFont-{suffix}");
            let Some(value) = VisualValue::optional_text(self.model.effective(&source)) else {
                continue;
            };
            let value = if self.model.has_dynamic_value(&source) {
                value
            } else {
                match suffix {
                    "size" => Self::parse_integer_prefix(&value),
                    "weight" => VisualValue::font_weight(&value),
                    "family" => VisualValue::quote(&value),
                    "style" => VisualValue::enum_value("FontStyle.", &value),
                    _ => value,
                }
            };
            values.push((target.to_string(), value.into()));
        }
        if !values.is_empty() {
            self.properties
                .set("placeholderFont", EtsValue::Object(values));
        }
    }

    fn parse_integer_prefix(value: &str) -> String {
        let value = value.trim_start();
        let sign = value.strip_prefix(['-', '+']).map_or("", |_| &value[..1]);
        let digits = value
            .trim_start_matches(['-', '+'])
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if digits.is_empty() {
            "NaN".to_string()
        } else {
            format!("{sign}{digits}")
        }
    }

    fn parse_navigation(&mut self) {
        self.string_property("navigation-title", "title");
        self.string_property("subTitle", "subTitle");
        self.raw_property("menus", "menus");
        self.enum_property("titleMode", "titleMode", "NavigationTitleMode.");
        self.raw_property("toolBar", "toolBar");
        self.raw_property("hideToolBar", "hideToolBar");
        self.raw_property("hideTitleBar", "hideTitleBar");
        self.raw_property("hideBackButton", "hideBackButton");
    }

    fn parse_time_picker(&mut self) {
        self.date_parameter("datetime-selected", "selected");
        self.raw_property("useMilitaryTime", "useMilitaryTime");
    }

    fn parse_date_picker(&mut self) {
        self.date_parameter("datetime-selected", "selected");
        self.date_parameter("start", "start");
        self.date_parameter("end", "end");
        self.raw_property("lunar", "lunar");
    }

    fn parse_text_picker(&mut self) {
        self.raw_parameter("range", "range");
        self.raw_parameter("textPicker-selected", "selected");
        self.string_property("defaultPickerItemHeight", "defaultPickerItemHeight");
    }

    fn date_parameter(&mut self, source: &str, target: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        let value = if self.model.has_dynamic_value(source) {
            value
        } else if value.is_empty() {
            "new Date()".to_string()
        } else {
            format!("new Date ({})", VisualValue::quote(&value))
        };
        self.set_parameter(target, value);
    }

    fn parse_refresh(&mut self) {
        self.raw_parameter("friction", "friction");
        self.string_parameter("offset", "offset");
        self.raw_parameter("refreshing", "refreshing");
    }

    fn parse_toggle(&mut self) {
        self.enum_parameter("toggle-type", "type", "ToggleType.");
        self.raw_parameter("isOn", "isOn");
        self.color_property("selectedColor", "selectedColor");
        self.color_property("switchPointColor", "switchPointColor");
    }

    fn parse_select(&mut self) {
        self.set_text_parameters(
            VisualValue::optional_text(self.model.dynamic("select-option"))
                .unwrap_or_else(|| "[{value: \"\"}]".to_string()),
        );
        if let Some(value) = VisualValue::optional_text(self.model.effective("select-value")) {
            let value = if self.model.has_dynamic_value("select-value") {
                VisualValue::content_expression(&value)
            } else {
                VisualValue::quote(&value)
            };
            self.properties.set("value", value);
        }
        self.raw_property("selected", "selected");
        for (source, group, member) in [
            ("font-size", "font", "size"),
            ("font-style", "font", "style"),
            ("font-weight", "font", "weight"),
            ("font-family", "font", "family"),
            ("selected-option-font-size", "selectedOptionFont", "size"),
            ("selected-option-font-style", "selectedOptionFont", "style"),
            (
                "selected-option-font-weight",
                "selectedOptionFont",
                "weight",
            ),
            (
                "selected-option-font-family",
                "selectedOptionFont",
                "family",
            ),
        ] {
            self.select_font(source, group, member);
        }
        self.color_property("selected-option-font-color", "selectedOptionFontColor");
        self.color_property("selected-option-bg-color", "selectedOptionBgColor");
        for (source, member) in [
            ("option-font-size", "size"),
            ("option-font-style", "style"),
            ("option-font-weight", "weight"),
            ("option-font-family", "family"),
        ] {
            self.select_font(source, "optionFont", member);
        }
        self.color_property("option-font-color", "optionFontColor");
        self.color_property("option-bg-color", "optionBgColor");
    }

    fn select_font(&mut self, source: &str, group: &str, member: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        let value = if self.model.has_dynamic_value(source) {
            value
        } else {
            match member {
                "size" | "family" => VisualValue::quote(&value),
                "style" => VisualValue::enum_value("FontStyle.", &value),
                "weight" => VisualValue::font_weight(&value),
                _ => value,
            }
        };
        let mut object = self
            .properties
            .0
            .iter_mut()
            .find_map(|(name, value)| (name == group).then_some(value));
        if let Some(EtsValue::Object(values)) = object.as_mut() {
            values.push((member.to_string(), value.into()));
        } else {
            self.properties.set(
                group,
                EtsValue::Object(vec![(member.to_string(), value.into())]),
            );
        }
    }

    fn parse_scroll(&mut self) {
        self.enum_property("scrollable", "scrollable", "ScrollDirection.");
        self.enum_property("scroll-bar", "scrollBar", "BarState.");
        self.color_property("scroll-bar-color", "scrollBarColor");
        self.string_property("scroll-bar-width", "scrollBarWidth");
        self.enum_property("edge-effect-scroll", "edgeEffect", "EdgeEffect.");
        if let Some(value) = VisualValue::optional_text(self.model.dynamic("scroller"))
            .filter(|value| !value.is_empty())
        {
            self.set_text_parameters(value);
        }
    }

    fn parse_grid(&mut self) {
        if let Some(value) = VisualValue::optional_text(self.model.dynamic("scroller"))
            .filter(|value| !value.is_empty())
        {
            self.set_text_parameters(value);
        }
        for source in [
            "columns-template",
            "rows-template",
            "columns-gap",
            "rows-gap",
        ] {
            self.string_property(source, &VisualValue::identifier(source));
        }
        self.enum_property("scroll-bar", "scrollBar", "BarState.");
        self.color_property("scroll-bar-color", "scrollBarColor");
        self.string_property("scroll-bar-width", "scrollBarWidth");
        self.raw_property("cached-count", "cachedCount");
        self.raw_property("edit-mode", "editMode");
        self.enum_property("layout-direction", "layoutDirection", "GridDirection.");
        for source in ["max-count", "min-count", "cell-length"] {
            self.raw_property(source, &VisualValue::identifier(source));
        }
        for source in ["multi-selectable", "support-animation"] {
            self.raw_property(source, &VisualValue::identifier(source));
        }
    }

    fn parse_grid_item(&mut self) {
        for source in ["row-start", "row-end", "column-start", "column-end"] {
            self.raw_property(source, &VisualValue::identifier(source));
        }
        self.raw_property("force-rebuild", "forceRebuild");
        self.raw_property("selectable", "selectable");
    }

    fn parse_search(&mut self) {
        for (source, group, member) in [
            ("text-font-size", "textFont", "size"),
            ("text-font-style", "textFont", "style"),
            ("text-font-weight", "textFont", "weight"),
            ("text-font-family", "textFont", "family"),
            ("placeholder-font-size", "placeholderFont", "size"),
            ("placeholder-font-style", "placeholderFont", "style"),
            ("placeholder-font-weight", "placeholderFont", "weight"),
            ("placeholder-font-family", "placeholderFont", "family"),
        ] {
            self.select_font(source, group, member);
        }
        self.color_property("placeholderColor", "placeholderColor");
        self.enum_property("search-text-align", "textAlign", "TextAlign.");
        let mut params = Vec::new();
        for (source, target) in [
            ("search-value", "value"),
            ("search-placeholder", "placeholder"),
            ("icon", "icon"),
            ("search-controller", "controller"),
        ] {
            let dynamic = self
                .model
                .dynamic(source)
                .filter(|value| !VisualValue::is_empty(Some(value)));
            let value = dynamic.or_else(|| self.model.property(source));
            let Some(value) = VisualValue::optional_text(value) else {
                continue;
            };
            let value = if dynamic.is_some() {
                if target == "controller" {
                    value
                } else {
                    VisualValue::content_expression(&value)
                }
            } else {
                VisualValue::quote(&value)
            };
            params.push(format!("{target}: {value}"));
        }
        self.set_text_parameters(format!("{{{}}}", params.join(", ")));
        self.string_property("search-button", "searchButton");
        self.enum_property("copy-option", "copyOption", "CopyOptions.");
    }

    fn parse_stack(&mut self) {
        self.enum_parameter("align-content-stack", "alignContent", "Alignment.");
    }

    fn parse_tabs(&mut self) {
        self.enum_parameter("bar-position", "barPosition", "BarPosition.");
        self.raw_parameter("index", "index");
        self.raw_parameter("controller", "controller");
        self.raw_property("vertical", "vertical");
        let dynamic_scrollable = VisualValue::optional_text(self.model.dynamic("tabs-scrollable"))
            .filter(|value| !value.trim().is_empty());
        if let Some(value) = dynamic_scrollable.or_else(|| {
            VisualValue::optional_text(self.model.property("tabs-scrollable"))
                .filter(|value| !value.is_empty())
        }) {
            self.properties.set("scrollable", value);
        }
        self.enum_property("bar-mode", "barMode", "BarMode.");
        self.string_property("bar-width", "barWidth");
        self.string_property("bar-height", "barHeight");
        self.raw_property("animation-duration", "animationDuration");
    }

    fn parse_tab_content(&mut self) {
        if !self.model.has("tabBar-icon") && !self.model.has("tabBar-text") {
            return;
        }
        let mut values = Vec::new();
        if let Some(icon) = VisualValue::optional_text(self.model.effective("tabBar-icon"))
            .filter(|value| !value.is_empty())
        {
            values.push((
                "icon".to_string(),
                if self.model.dynamic("tabBar-icon").is_some() {
                    icon
                } else {
                    VisualValue::source(&icon)
                }
                .into(),
            ));
        }
        let text = VisualValue::optional_text(self.model.effective("tabBar-text"))
            .unwrap_or_else(|| "TabBar".to_string());
        values.push((
            "text".to_string(),
            if self.model.dynamic("tabBar-text").is_some() {
                text
            } else {
                VisualValue::quote(&text)
            }
            .into(),
        ));
        if !values.is_empty() {
            self.properties.set("tabBar", EtsValue::Object(values));
        }
    }

    fn parse_checkbox(&mut self) {
        self.raw_property("select", "select");
        self.color_property("selectedColor", "selectedColor");
        self.string_parameter("name", "name");
        self.string_parameter("group", "group");
    }

    fn parse_checkbox_group(&mut self) {
        self.raw_property("selectAll", "selectAll");
        self.color_property("selectedColor", "selectedColor");
        self.string_parameter("group", "group");
    }

    fn parse_custom(&mut self) {
        let Some(component) = self.context.component(&self.model.tag_type).cloned() else {
            return;
        };
        for attribute in component.attributes {
            if self.model.has(&attribute.source) {
                let dynamic = self
                    .model
                    .dynamic(&attribute.source)
                    .filter(|value| !VisualValue::is_empty(Some(value)));
                let value = dynamic.or_else(|| self.model.property(&attribute.source));
                if let Some(value) =
                    VisualValue::optional_text(value).filter(|value| !value.is_empty())
                {
                    let value = if dynamic.is_some() {
                        if attribute.decorator == "@Link" {
                            value.replacen("this.", "$", 1)
                        } else {
                            value
                        }
                    } else {
                        Self::custom_value(&attribute.value_type, &value)
                    };
                    self.set_parameter(&attribute.target, value);
                }
            }
            if self.model.has_media(&attribute.source) {
                self.custom_media_parameter(&attribute);
            }
        }
        for event in component.events {
            let Some(mut value) = VisualValue::optional_text(self.model.property(&event)) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            if value.split('.').next() == Some("this") {
                value.push_str(".bind(this)");
            }
            self.set_parameter(&event, value);
        }
    }

    fn custom_value(value_type: &str, value: &str) -> String {
        match value_type {
            "string" => VisualValue::quote(value),
            "boolean" | "number" | "any[]" | "object" => value.to_string(),
            "Date" => {
                if value.is_empty() {
                    "new Date()".to_string()
                } else {
                    format!("new Date ({})", VisualValue::quote(value))
                }
            }
            "Alignment" => {
                let alignment = match value {
                    "top left" => Some("TopStart"),
                    "top center" => Some("Top"),
                    "top right" => Some("TopEnd"),
                    "center left" => Some("Start"),
                    "center center" => Some("Center"),
                    "center right" => Some("End"),
                    "bottom left" => Some("BottomStart"),
                    "bottom center" => Some("Bottom"),
                    "bottom right" => Some("BottomEnd"),
                    _ => None,
                };
                alignment.map_or_else(
                    || VisualValue::enum_value("Alignment.", value),
                    |alignment| format!("Alignment.{alignment}"),
                )
            }
            "ItemAlign" => MediaValueKind::ItemAlign.transform(value),
            "HorizontalAlign" => MediaValueKind::HorizontalAlign.transform(value),
            "VerticalAlign" => MediaValueKind::VerticalAlign.transform(value),
            "FlexAlign" => MediaValueKind::FlexAlign.transform(value),
            "FlexWrap" => MediaValueKind::Wrap.transform(value),
            "FontWeight" => VisualValue::font_weight(value),
            "ImageRepeat" => match value {
                "repeat-x" => "ImageRepeat.X".to_string(),
                "repeat-y" => "ImageRepeat.Y".to_string(),
                "repeat" => "ImageRepeat.XY".to_string(),
                "no-repeat" => "ImageRepeat.NoRepeat".to_string(),
                _ => VisualValue::enum_value("ImageRepeat.", value),
            },
            value_type if !value_type.is_empty() => {
                VisualValue::enum_value(&format!("{value_type}."), value)
            }
            _ => value.to_string(),
        }
    }

    fn custom_media_parameter(&mut self, attribute: &CustomAttribute) {
        let variable = format!(
            "this.{}{}_{}",
            VisualValue::identifier(&self.model.id),
            VisualValue::upper_first(&VisualValue::identifier(&attribute.source)),
            VisualValue::unique_id(&format!(
                "this.{}{}",
                VisualValue::identifier(&self.model.id),
                VisualValue::upper_first(&VisualValue::identifier(&attribute.source))
            ))
        );
        let decorator = if attribute.decorator == "@Link" || attribute.decorator == "@Prop" {
            "@State"
        } else {
            ""
        };
        self.defaults.push(VisualMediaDefault {
            variable: variable.clone(),
            value_type: attribute.value_type.clone(),
            initial: VisualValue::optional_text(self.model.dynamic(&attribute.source))
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    VisualValue::optional_text(self.model.property(&attribute.source))
                        .filter(|value| !value.is_empty())
                        .map(|value| Self::custom_value(&attribute.value_type, &value))
                })
                .unwrap_or_else(|| "undefined".to_string()),
            decorator: decorator.to_string(),
        });
        for (query, properties) in &self.model.media_properties.0 {
            if let Some(value) = VisualValue::optional_text(properties.get(&attribute.source))
                .filter(|value| !value.is_empty())
            {
                self.media_assignments.push(VisualMediaAssignment {
                    query: query.clone(),
                    variable: variable.clone(),
                    value: Self::custom_value(&attribute.value_type, &value),
                });
            }
        }
        for (query, properties) in &self.model.dynamic_media_properties.0 {
            if let Some(value) = VisualValue::optional_text(properties.get(&attribute.source))
                .filter(|value| !value.is_empty())
            {
                let assignment = VisualMediaAssignment {
                    query: query.clone(),
                    variable: variable.clone(),
                    value,
                };
                if let Some(current) = self.media_assignments.iter_mut().find(|current| {
                    current.query == assignment.query && current.variable == assignment.variable
                }) {
                    *current = assignment;
                } else {
                    self.media_assignments.push(assignment);
                }
            }
        }
        self.set_parameter(
            &attribute.target,
            if attribute.decorator == "@Link" {
                variable.replacen("this.", "$", 1)
            } else {
                variable
            },
        );
    }

    fn parse_media(&mut self) {
        if let Some(variable) = self.media_binding("for", MediaValueKind::Raw) {
            self.media_for_each = Some(ForEachModel {
                items: variable,
                key: VisualValue::optional_text(self.model.dynamic("keyGenerator")),
                item: VisualValue::optional_text(self.model.property("item"))
                    .unwrap_or_else(|| "item".to_string()),
                index: VisualValue::optional_text(self.model.property("idx"))
                    .unwrap_or_else(|| "idx".to_string()),
            });
        }
        if let Some(variable) = self.media_binding("if", MediaValueKind::Raw) {
            self.media_condition = Some(variable);
        }
        self.media_property("width", "width", MediaValueKind::String);
        self.media_property("height", "height", MediaValueKind::String);
        self.media_constraint_size();
        self.media_property("align", "align", MediaValueKind::Enum("Alignment."));
        self.media_property("direction", "direction", MediaValueKind::Enum("Direction."));
        self.media_position();
        self.media_property("aspect-ratio", "aspectRatio", MediaValueKind::Raw);
        self.media_property("display-priority", "displayPriority", MediaValueKind::Raw);
        self.media_property("flex-basis", "flexBasis", MediaValueKind::String);
        self.media_property("flex-grow", "flexGrow", MediaValueKind::Raw);
        self.media_property("flex-shrink", "flexShrink", MediaValueKind::Raw);
        self.media_property("align-self", "alignSelf", MediaValueKind::ItemAlign);
        self.media_property(
            "border-style",
            "borderStyle",
            MediaValueKind::Enum("BorderStyle."),
        );
        self.media_border_edges("border-style", MediaValueKind::Enum("BorderStyle."));
        self.media_property("border-width", "borderWidth", MediaValueKind::String);
        self.media_border_edges("border-width", MediaValueKind::String);
        self.media_property("border-color", "borderColor", MediaValueKind::Color);
        self.media_border_edges("border-color", MediaValueKind::Color);
        self.media_property("border-radius", "borderRadius", MediaValueKind::String);
        self.media_border_edges("border-radius", MediaValueKind::String);
        self.media_property("background-color", "backgroundColor", MediaValueKind::Color);
        self.media_background_image();
        self.media_background_image_size();
        self.media_background_image_position();
        self.media_property("opacity", "opacity", MediaValueKind::Raw);
        self.media_property(
            "visibility",
            "visibility",
            MediaValueKind::Enum("Visibility."),
        );
        self.media_property("enabled", "enabled", MediaValueKind::Raw);
        self.media_property("font-color", "fontColor", MediaValueKind::Color);
        self.media_box_edges("margin");
        self.media_box_edges("padding");

        match self.model.tag_type.as_str() {
            "button" => {
                self.media_text_parameters("label", MediaValueKind::Content);
                self.media_property("type", "type", MediaValueKind::Enum("ButtonType."));
                self.media_property("state-effect", "stateEffect", MediaValueKind::Raw);
                self.media_property("font-size", "fontSize", MediaValueKind::String);
                self.media_property(
                    "font-style",
                    "fontStyle",
                    MediaValueKind::Enum("FontStyle."),
                );
                self.media_property("font-weight", "fontWeight", MediaValueKind::FontWeight);
                self.media_property("font-family", "fontFamily", MediaValueKind::String);
            }
            "divider" => {
                self.media_property("vertical", "vertical", MediaValueKind::Raw);
                self.media_property("color", "color", MediaValueKind::Color);
                self.media_property("stroke-width", "strokeWidth", MediaValueKind::String);
                self.media_property("line-cap", "lineCap", MediaValueKind::Enum("LineCapStyle."));
            }
            "image" => {
                self.media_text_parameters("src", MediaValueKind::Source);
                self.media_property("alt", "alt", MediaValueKind::String);
                self.media_property("object-fit", "objectFit", MediaValueKind::Enum("ImageFit."));
                self.media_property(
                    "object-repeat",
                    "objectRepeat",
                    MediaValueKind::Enum("ImageRepeat."),
                );
                self.media_property(
                    "interpolation",
                    "interpolation",
                    MediaValueKind::Enum("ImageInterpolation."),
                );
                self.media_property(
                    "render-mode",
                    "renderMode",
                    MediaValueKind::Enum("ImageRenderMode."),
                );
                self.media_image_source_size();
            }
            "progress" => {
                self.media_parameter("value", "value", MediaValueKind::Raw);
                self.media_parameter("total", "total", MediaValueKind::Raw);
                self.media_parameter("style", "style", MediaValueKind::Enum("ProgressStyle."));
                self.media_property("color", "color", MediaValueKind::Color);
            }
            "slider" => {
                for source in ["value", "min", "max", "step"] {
                    self.media_parameter(source, source, MediaValueKind::Raw);
                }
                self.media_parameter("style", "style", MediaValueKind::Enum("SliderStyle."));
                for (source, target) in [
                    ("block-color", "blockColor"),
                    ("track-color", "trackColor"),
                    ("selected-color", "selectedColor"),
                ] {
                    self.media_property(source, target, MediaValueKind::Color);
                }
                self.media_property("show-steps", "showSteps", MediaValueKind::Raw);
                self.media_property("show-tips", "showTips", MediaValueKind::Raw);
            }
            "text" => {
                self.media_text_parameters("content", MediaValueKind::Content);
                self.media_property(
                    "text-align",
                    "textAlign",
                    MediaValueKind::Enum("TextAlign."),
                );
                self.media_text_overflow();
                self.media_property("max-lines", "maxLines", MediaValueKind::Raw);
                self.media_property("line-height", "lineHeight", MediaValueKind::String);
                self.media_text_decoration();
                self.media_property("baseline-offset", "baselineOffset", MediaValueKind::String);
                self.media_property("text-case", "textCase", MediaValueKind::Enum("TextCase."));
                self.media_property("font-size", "fontSize", MediaValueKind::String);
                self.media_property(
                    "font-style",
                    "fontStyle",
                    MediaValueKind::Enum("FontStyle."),
                );
                self.media_property("font-weight", "fontWeight", MediaValueKind::FontWeight);
                self.media_property("font-family", "fontFamily", MediaValueKind::String);
            }
            "column" => {
                self.media_parameter("space", "space", MediaValueKind::String);
                self.media_property(
                    "align-items-column",
                    "alignItems",
                    MediaValueKind::HorizontalAlign,
                );
                self.media_property(
                    "justify-content-rc",
                    "justifyContent",
                    MediaValueKind::FlexAlign,
                );
            }
            "row" => {
                self.media_parameter("space", "space", MediaValueKind::String);
                self.media_property(
                    "align-items-row",
                    "alignItems",
                    MediaValueKind::VerticalAlign,
                );
                self.media_property(
                    "justify-content-rc",
                    "justifyContent",
                    MediaValueKind::FlexAlign,
                );
            }
            "flex" => {
                self.media_parameter(
                    "flex-direction",
                    "direction",
                    MediaValueKind::Enum("FlexDirection."),
                );
                self.media_parameter("wrap", "wrap", MediaValueKind::Wrap);
                self.media_parameter(
                    "justify-content",
                    "justifyContent",
                    MediaValueKind::FlexAlign,
                );
                self.media_parameter("align-items-flex", "alignItems", MediaValueKind::ItemAlign);
                self.media_parameter("align-content", "alignContent", MediaValueKind::FlexAlign);
            }
            "list" => {
                self.media_parameter("space", "space", MediaValueKind::String);
                self.media_parameter("initial-index", "initialIndex", MediaValueKind::Raw);
                self.media_property(
                    "list-direction",
                    "listDirection",
                    MediaValueKind::Enum("Axis."),
                );
                self.media_list_divider();
                self.media_property("edit-mode", "editMode", MediaValueKind::Raw);
                self.media_property(
                    "edge-effect",
                    "edgeEffect",
                    MediaValueKind::Enum("EdgeEffect."),
                );
                self.media_property("chain-animation", "chainAnimation", MediaValueKind::Raw);
            }
            "list-item" => {
                self.media_property("sticky", "sticky", MediaValueKind::Enum("Sticky."));
                self.media_property("editable", "editable", MediaValueKind::Raw);
            }
            "swiper" => {
                for source in ["index", "interval", "duration", "cached-count"] {
                    self.media_property(
                        source,
                        &VisualValue::identifier(source),
                        MediaValueKind::Raw,
                    );
                }
                for source in [
                    "auto-play",
                    "indicator",
                    "loop",
                    "vertical",
                    "disable-swipe",
                ] {
                    self.media_property(
                        source,
                        &VisualValue::identifier(source),
                        MediaValueKind::Raw,
                    );
                }
                self.media_property("item-space", "itemSpace", MediaValueKind::String);
            }
            "text-input" => {
                self.media_parameter("placeholder", "placeholder", MediaValueKind::String);
                self.media_property("textInput-type", "type", MediaValueKind::Enum("InputType."));
                self.media_property(
                    "placeholderColor",
                    "placeholderColor",
                    MediaValueKind::Color,
                );
                self.media_placeholder_font();
                self.media_property(
                    "enterKeyType",
                    "enterKeyType",
                    MediaValueKind::Enum("EnterKeyType."),
                );
                self.media_property("caretColor", "caretColor", MediaValueKind::Color);
                self.media_property("maxLength", "maxLength", MediaValueKind::Raw);
            }
            "navigation" => {
                self.media_property("navigation-title", "title", MediaValueKind::String);
                self.media_property("subTitle", "subTitle", MediaValueKind::String);
                self.media_property("menus", "menus", MediaValueKind::Raw);
                self.media_property(
                    "titleMode",
                    "titleMode",
                    MediaValueKind::Enum("NavigationTitleMode."),
                );
                self.media_property("toolBar", "toolBar", MediaValueKind::Raw);
                for source in ["hideToolBar", "hideTitleBar", "hideBackButton"] {
                    self.media_property(source, source, MediaValueKind::Raw);
                }
            }
            "timePicker" => {
                self.media_date_parameter("datetime-selected", "selected", "private");
                self.media_property("useMilitaryTime", "useMilitaryTime", MediaValueKind::Raw);
            }
            "datePicker" => {
                self.media_date_parameter("datetime-selected", "selected", "private");
                self.media_date_parameter("start", "start", "private");
                self.media_date_parameter("end", "end", "private");
                self.media_property("lunar", "lunar", MediaValueKind::Raw);
            }
            "textPicker" => {
                self.media_parameter("range", "range", MediaValueKind::Raw);
                self.media_parameter_with_decorator(
                    "textPicker-selected",
                    "selected",
                    MediaValueKind::Raw,
                    "@State",
                );
                self.media_property(
                    "defaultPickerItemHeight",
                    "defaultPickerItemHeight",
                    MediaValueKind::String,
                );
            }
            "refresh" => {
                self.media_parameter("friction", "friction", MediaValueKind::Raw);
                self.media_parameter("offset", "offset", MediaValueKind::String);
                self.media_parameter("refreshing", "refreshing", MediaValueKind::Raw);
            }
            "toggle" => {
                self.media_parameter("toggle-type", "type", MediaValueKind::Enum("ToggleType."));
                self.media_parameter("isOn", "isOn", MediaValueKind::Raw);
                self.media_property("selectedColor", "selectedColor", MediaValueKind::Color);
                self.media_property(
                    "switchPointColor",
                    "switchPointColor",
                    MediaValueKind::Color,
                );
            }
            "select" => {
                self.media_select_options();
                self.media_property("select-value", "value", MediaValueKind::String);
                self.media_property("selected", "selected", MediaValueKind::Raw);
                self.media_font_group(
                    "font",
                    &[
                        ("font-size", "size"),
                        ("font-style", "style"),
                        ("font-family", "family"),
                        ("font-weight", "weight"),
                    ],
                );
                self.media_font_group(
                    "selectedOptionFont",
                    &[
                        ("selected-option-font-size", "size"),
                        ("selected-option-font-style", "style"),
                        ("selected-option-font-family", "family"),
                        ("selected-option-font-weight", "weight"),
                    ],
                );
                self.media_property(
                    "selected-option-font-color",
                    "selectedOptionFontColor",
                    MediaValueKind::Color,
                );
                self.media_property(
                    "selected-option-bg-color",
                    "selectedOptionBgColor",
                    MediaValueKind::Color,
                );
                self.media_font_group(
                    "optionFont",
                    &[
                        ("option-font-size", "size"),
                        ("option-font-style", "style"),
                        ("option-font-family", "family"),
                        ("option-font-weight", "weight"),
                    ],
                );
                self.media_property(
                    "option-font-color",
                    "optionFontColor",
                    MediaValueKind::Color,
                );
                self.media_property("option-bg-color", "optionBgColor", MediaValueKind::Color);
            }
            "scroll" => {
                self.media_property(
                    "scrollable",
                    "scrollable",
                    MediaValueKind::Enum("ScrollDirection."),
                );
                self.media_property("scroll-bar", "scrollBar", MediaValueKind::Enum("BarState."));
                self.media_property("scroll-bar-color", "scrollBarColor", MediaValueKind::Color);
                self.media_property("scroll-bar-width", "scrollBarWidth", MediaValueKind::String);
                self.media_property(
                    "edge-effect-scroll",
                    "edgeEffect",
                    MediaValueKind::Enum("EdgeEffect."),
                );
            }
            "grid" => {
                for source in [
                    "columns-template",
                    "rows-template",
                    "columns-gap",
                    "rows-gap",
                    "scroll-bar-width",
                ] {
                    self.media_property(
                        source,
                        &VisualValue::identifier(source),
                        MediaValueKind::String,
                    );
                }
                self.media_property("scroll-bar", "scrollBar", MediaValueKind::Enum("BarState."));
                self.media_property("scroll-bar-color", "scrollBarColor", MediaValueKind::Color);
                for source in [
                    "cached-count",
                    "max-count",
                    "min-count",
                    "cell-length",
                    "edit-mode",
                    "multi-selectable",
                    "support-animation",
                ] {
                    self.media_property(
                        source,
                        &VisualValue::identifier(source),
                        MediaValueKind::Raw,
                    );
                }
                self.media_property(
                    "layout-direction",
                    "layoutDirection",
                    MediaValueKind::Enum("GridDirection."),
                );
            }
            "grid-item" => {
                for source in [
                    "row-start",
                    "row-end",
                    "column-start",
                    "column-end",
                    "force-rebuild",
                    "selectable",
                ] {
                    self.media_property(
                        source,
                        &VisualValue::identifier(source),
                        MediaValueKind::Raw,
                    );
                }
            }
            "stack" => self.media_parameter(
                "align-content-stack",
                "alignContent",
                MediaValueKind::Enum("Alignment."),
            ),
            "search" => self.media_search(),
            "tabs" => {
                self.media_parameter(
                    "bar-position",
                    "barPosition",
                    MediaValueKind::Enum("BarPosition."),
                );
                self.media_parameter("index", "index", MediaValueKind::Raw);
                self.media_parameter("controller", "controller", MediaValueKind::Raw);
                self.media_property("vertical", "vertical", MediaValueKind::Raw);
                self.media_property("tabs-scrollable", "scrollable", MediaValueKind::Raw);
                self.media_property("bar-mode", "barMode", MediaValueKind::Enum("BarMode."));
                self.media_property("bar-width", "barWidth", MediaValueKind::String);
                self.media_property("bar-height", "barHeight", MediaValueKind::String);
                self.media_property(
                    "animation-duration",
                    "animationDuration",
                    MediaValueKind::Raw,
                );
            }
            "tab-content" => self.media_tab_content(),
            "checkbox" => {
                self.media_property("select", "select", MediaValueKind::Raw);
                self.media_property("selectedColor", "selectedColor", MediaValueKind::Color);
                self.media_parameter("name", "name", MediaValueKind::String);
                self.media_parameter("group", "group", MediaValueKind::String);
            }
            "checkboxGroup" => {
                self.media_property("selectAll", "selectAll", MediaValueKind::Raw);
                self.media_property("selectedColor", "selectedColor", MediaValueKind::Color);
                self.media_parameter("group", "group", MediaValueKind::String);
            }
            _ => {}
        }
    }

    fn media_constraint_size(&mut self) {
        let mut values = Vec::new();
        for (source, target) in [
            ("constraint-size-min-width", "minWidth"),
            ("constraint-size-max-width", "maxWidth"),
            ("constraint-size-min-height", "minHeight"),
            ("constraint-size-max-height", "maxHeight"),
        ] {
            if let Some(variable) = self.media_binding(source, MediaValueKind::String) {
                values.push((target.to_string(), variable.into()));
            }
        }
        if !values.is_empty() {
            self.properties
                .set("constraintSize", EtsValue::Object(values));
        }
    }

    fn media_position(&mut self) {
        if !self.model.has_media("left") && !self.model.has_media("top") {
            return;
        }
        let base_left = self.media_position_default("left");
        let base_top = self.media_position_default("top");
        let base_mode = VisualValue::optional_text(self.model.property("position"))
            .unwrap_or_else(|| "relative".to_string());
        let mut position_left = Vec::new();
        let mut position_top = Vec::new();
        let mut offset_left = Vec::new();
        let mut offset_top = Vec::new();
        for query in self.media_queries() {
            let static_properties = self.model.media_properties.get(&query);
            let dynamic_properties = self.model.dynamic_media_properties.get(&query);
            let mode = static_properties
                .and_then(|properties| VisualValue::optional_text(properties.get("position")))
                .unwrap_or_else(|| "relative".to_string());
            let (left, top) = self.media_position_values(
                static_properties,
                dynamic_properties,
                &base_left,
                &base_top,
            );
            let (left_values, top_values) = if mode == "absolute" {
                (&mut position_left, &mut position_top)
            } else {
                (&mut offset_left, &mut offset_top)
            };
            if left != "undefined" {
                left_values.push((query.clone(), left));
            }
            if top != "undefined" {
                top_values.push((query, top));
            }
        }
        if position_left.is_empty()
            && position_top.is_empty()
            && offset_left.is_empty()
            && offset_top.is_empty()
        {
            return;
        }
        let has_position =
            !position_left.is_empty() || !position_top.is_empty() || base_mode == "absolute";
        let has_offset =
            !offset_left.is_empty() || !offset_top.is_empty() || base_mode == "relative";
        if has_position && has_offset {
            Self::fill_media_resets(&mut position_left, &offset_left);
            Self::fill_media_resets(&mut offset_left, &position_left);
            Self::fill_media_resets(&mut position_top, &offset_top);
            Self::fill_media_resets(&mut offset_top, &position_top);
        }
        if has_position {
            let left = self.register_media_variable(
                "positionLeft",
                if base_mode == "absolute" {
                    base_left.clone()
                } else {
                    "undefined".to_string()
                },
                Self::media_type("left"),
                position_left,
                "",
            );
            let top = self.register_media_variable(
                "positionTop",
                if base_mode == "absolute" {
                    base_top.clone()
                } else {
                    "undefined".to_string()
                },
                Self::media_type("top"),
                position_top,
                "",
            );
            self.properties.set(
                "position",
                EtsValue::Object(vec![
                    ("x".to_string(), left.into()),
                    ("y".to_string(), top.into()),
                ]),
            );
        }
        if has_offset {
            let left = self.register_media_variable(
                "offsetLeft",
                if base_mode == "relative" {
                    base_left
                } else {
                    "undefined".to_string()
                },
                Self::media_type("left"),
                offset_left,
                "",
            );
            let top = self.register_media_variable(
                "offsetTop",
                if base_mode == "relative" {
                    base_top
                } else {
                    "undefined".to_string()
                },
                Self::media_type("top"),
                offset_top,
                "",
            );
            self.properties.set(
                "offset",
                EtsValue::Object(vec![
                    ("x".to_string(), left.into()),
                    ("y".to_string(), top.into()),
                ]),
            );
        }
    }

    fn media_position_default(&self, source: &str) -> String {
        if let Some(value) = VisualValue::optional_text(self.model.dynamic(source)) {
            return value;
        }
        VisualValue::optional_text(self.model.property(source)).map_or_else(
            || "undefined".to_string(),
            |value| VisualValue::quote(&value),
        )
    }

    fn media_position_values(
        &self,
        static_properties: Option<&PropertyMap>,
        dynamic_properties: Option<&PropertyMap>,
        base_left: &str,
        base_top: &str,
    ) -> (String, String) {
        let mut left = static_properties
            .and_then(|properties| VisualValue::optional_text(properties.get("left")))
            .map_or_else(|| base_left.to_string(), |value| VisualValue::quote(&value));
        let mut top = static_properties
            .and_then(|properties| VisualValue::optional_text(properties.get("top")))
            .map_or_else(|| base_top.to_string(), |value| VisualValue::quote(&value));
        if let Some(value) = dynamic_properties
            .and_then(|properties| VisualValue::optional_text(properties.get("left")))
        {
            left = value;
        }
        if let Some(value) = dynamic_properties
            .and_then(|properties| VisualValue::optional_text(properties.get("top")))
        {
            top = value;
        }
        (left, top)
    }

    fn fill_media_resets(target: &mut Vec<(String, String)>, source: &[(String, String)]) {
        for (query, _) in source {
            if !target.iter().any(|(current, _)| current == query) {
                target.push((query.clone(), "undefined".to_string()));
            }
        }
    }

    fn media_border_edges(&mut self, source: &str, kind: MediaValueKind) {
        let suffix = source.trim_start_matches("border-");
        let edges = if suffix == "radius" {
            vec![
                ("border-top-left-radius".to_string(), "topLeft"),
                ("border-bottom-right-radius".to_string(), "bottomRight"),
                ("border-bottom-left-radius".to_string(), "bottomLeft"),
                ("border-top-right-radius".to_string(), "topRight"),
            ]
        } else {
            vec![
                (format!("border-top-{suffix}"), "top"),
                (format!("border-bottom-{suffix}"), "bottom"),
                (format!("border-left-{suffix}"), "left"),
                (format!("border-right-{suffix}"), "right"),
            ]
        };
        if !edges.iter().any(|(edge, _)| self.model.has_media(edge)) {
            return;
        }
        let base_default = self.media_default(source, kind);
        let base_assignments = self.media_values(source, kind);
        let mut values = Vec::new();
        for (edge, target) in edges {
            let own_assignments = self.media_values(&edge, kind);
            let initial = if self.model.has(&edge) {
                self.media_default(&edge, kind)
            } else {
                base_default.clone()
            };
            let variable = self.register_media_variable(
                &edge,
                initial,
                Self::media_type(&edge),
                if own_assignments.is_empty() {
                    base_assignments.clone()
                } else {
                    own_assignments
                },
                "",
            );
            values.push((target.to_string(), variable.into()));
        }
        self.properties
            .set(VisualValue::identifier(source), EtsValue::Object(values));
    }

    fn media_background_image(&mut self) {
        if !self.model.has_media("background-image-src")
            && !self.model.has_media("background-image-repeat")
        {
            return;
        }
        let src = if self.model.has_media("background-image-src") {
            self.media_binding("background-image-src", MediaValueKind::Source)
                .expect("checked media source")
        } else {
            self.media_default("background-image-src", MediaValueKind::Source)
        };
        let repeat = if self.model.has_media("background-image-repeat") {
            self.media_binding("background-image-repeat", MediaValueKind::ImageRepeat)
                .expect("checked media repeat")
        } else {
            self.media_default("background-image-repeat", MediaValueKind::ImageRepeat)
        };
        self.properties
            .set("backgroundImage", format!("{src}, {repeat}"));
    }

    fn media_background_image_size(&mut self) {
        self.media_background_dual_value(
            "background-image-size",
            "backgroundImageSize",
            "ImageSize",
            "SizeOptions",
            Self::background_image_size_enum,
            Self::background_image_size_object,
        );
    }

    fn media_background_image_position(&mut self) {
        self.media_background_dual_value(
            "background-image-position",
            "backgroundImagePosition",
            "Alignment",
            "Position",
            Self::background_image_position_enum,
            Self::background_image_position_object,
        );
    }

    fn media_background_dual_value(
        &mut self,
        source: &str,
        target: &str,
        enum_type: &str,
        object_type: &str,
        enum_transform: fn(&str) -> String,
        object_transform: fn(&str) -> String,
    ) {
        if !self.model.has_media(source) {
            return;
        }
        let static_default = VisualValue::optional_text(self.model.property(source));
        let dynamic_default = VisualValue::optional_text(self.model.dynamic(source));
        let (enum_default, object_default) = if let Some(dynamic) = dynamic_default {
            (dynamic, "undefined".to_string())
        } else if let Some(value) = static_default {
            let enum_value = enum_transform(&value);
            (
                if enum_value.is_empty() {
                    "undefined".to_string()
                } else {
                    enum_value
                },
                object_transform(&value),
            )
        } else {
            ("undefined".to_string(), "undefined".to_string())
        };
        let mut enum_assignments = Vec::new();
        let mut object_assignments = Vec::new();
        for (query, properties) in &self.model.media_properties.0 {
            let Some(value) = VisualValue::optional_text(properties.get(source))
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let enum_value = enum_transform(&value);
            enum_assignments.push((
                query.clone(),
                if enum_value.is_empty() {
                    "undefined".to_string()
                } else {
                    enum_value
                },
            ));
            object_assignments.push((query.clone(), object_transform(&value)));
        }
        for (query, properties) in &self.model.dynamic_media_properties.0 {
            let Some(value) = VisualValue::optional_text(properties.get(source))
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            Self::upsert_media_value(&mut enum_assignments, query, value);
            Self::upsert_media_value(&mut object_assignments, query, "undefined".to_string());
        }
        let enum_variable = self.register_media_variable(
            &format!("{source}-enum"),
            enum_default,
            enum_type,
            enum_assignments,
            "",
        );
        let object_variable = self.register_media_variable(
            &format!("{source}-obj"),
            object_default,
            object_type,
            object_assignments,
            "",
        );
        self.properties.set(
            target,
            format!("{enum_variable} !== undefined ? {enum_variable} : {object_variable}"),
        );
    }

    fn media_box_edges(&mut self, source: &str) {
        let edges = ["top", "bottom", "left", "right"];
        if !self.model.has_media(source)
            && !edges
                .iter()
                .any(|edge| self.model.has_media(&format!("{source}-{edge}")))
        {
            return;
        }
        let base_default = self.media_default(source, MediaValueKind::String);
        let mut values = Vec::new();
        let mut generated = 0;
        for edge in edges {
            let edge_source = format!("{source}-{edge}");
            let initial = if self.model.has(&edge_source) {
                self.media_default(&edge_source, MediaValueKind::String)
            } else {
                base_default.clone()
            };
            let assignments = self.media_edge_values(source, &edge_source);
            let value = if assignments.is_empty() {
                initial
            } else {
                generated += 1;
                self.register_media_variable(
                    &edge_source,
                    initial,
                    Self::media_type(&edge_source),
                    assignments,
                    "",
                )
            };
            if value != "undefined" {
                values.push((edge.to_string(), value.into()));
            }
        }
        if generated > 0 {
            self.properties.set(source, EtsValue::Object(values));
        }
    }

    fn media_edge_values(&self, shorthand: &str, edge: &str) -> Vec<(String, String)> {
        let mut values = Vec::new();
        for query in self.media_queries() {
            let static_properties = self.model.media_properties.get(&query);
            let dynamic_properties = self.model.dynamic_media_properties.get(&query);
            let static_value = static_properties.and_then(|properties| {
                VisualValue::optional_text(properties.get(edge))
                    .or_else(|| {
                        dynamic_properties.and_then(|properties| {
                            VisualValue::optional_text(properties.get(shorthand))
                        })
                    })
                    .or_else(|| VisualValue::optional_text(properties.get(shorthand)))
            });
            let dynamic_value = dynamic_properties.and_then(|properties| {
                VisualValue::optional_text(properties.get(edge))
                    .or_else(|| VisualValue::optional_text(properties.get(shorthand)))
            });
            if let Some(value) = dynamic_value {
                Self::upsert_media_value(&mut values, &query, value);
            } else if let Some(value) = static_value {
                Self::upsert_media_value(&mut values, &query, VisualValue::quote(&value));
            }
        }
        values
    }

    fn media_image_source_size(&mut self) {
        if !self.model.has_media("source-size-width") && !self.model.has_media("source-size-height")
        {
            return;
        }
        let width_default = self.media_source_size_default("source-size-width");
        let height_default = self.media_source_size_default("source-size-height");
        let width_assignments =
            self.media_source_size_values("source-size-width", "source-size-height");
        if width_assignments.is_empty() {
            return;
        }
        let height_assignments =
            self.media_source_size_values("source-size-height", "source-size-width");
        let width = self.register_media_variable(
            "source-size-width",
            width_default,
            Self::media_type("source-size-width"),
            width_assignments,
            "",
        );
        let height = self.register_media_variable(
            "source-size-height",
            height_default,
            Self::media_type("source-size-height"),
            height_assignments,
            "",
        );
        self.properties.set(
            "sourceSize",
            EtsValue::Object(vec![
                ("width".to_string(), width.into()),
                ("height".to_string(), height.into()),
            ]),
        );
    }

    fn media_source_size_default(&self, source: &str) -> String {
        if let Some(value) =
            VisualValue::optional_text(self.model.dynamic(source)).filter(|value| !value.is_empty())
        {
            return value;
        }
        VisualValue::optional_text(self.model.property(source)).map_or_else(
            || "0".to_string(),
            |value| Self::strip_source_size_unit(&value),
        )
    }

    fn media_source_size_values(&self, source: &str, paired: &str) -> Vec<(String, String)> {
        let mut values = Vec::new();
        for (query, properties) in &self.model.media_properties.0 {
            let value = VisualValue::optional_text(properties.get(source));
            let paired = VisualValue::optional_text(properties.get(paired));
            if value.as_deref().is_none_or(str::is_empty)
                && paired.as_deref().is_none_or(str::is_empty)
            {
                continue;
            }
            values.push((
                query.clone(),
                value.map_or_else(
                    || "0".to_string(),
                    |value| Self::strip_source_size_unit(&value),
                ),
            ));
        }
        for (query, properties) in &self.model.dynamic_media_properties.0 {
            if let Some(value) =
                VisualValue::optional_text(properties.get(source)).filter(|value| !value.is_empty())
            {
                Self::upsert_media_value(&mut values, query, value);
            }
        }
        values
    }

    fn strip_source_size_unit(value: &str) -> String {
        if value == "0" || value.is_empty() {
            "0".to_string()
        } else {
            value[..value.len().saturating_sub(2)].to_string()
        }
    }

    fn media_text_overflow(&mut self) {
        if let Some(variable) =
            self.media_binding("text-overflow", MediaValueKind::Enum("TextOverflow."))
        {
            self.properties.set(
                "textOverflow",
                EtsValue::Object(vec![("overflow".to_string(), variable.into())]),
            );
        }
    }

    fn media_text_decoration(&mut self) {
        if !self.model.has_media("decoration-type") && !self.model.has_media("decoration-color") {
            return;
        }
        let type_value = if self.model.has_media("decoration-type") {
            self.media_binding(
                "decoration-type",
                MediaValueKind::Enum("TextDecorationType."),
            )
            .expect("checked decoration type")
        } else {
            self.media_default(
                "decoration-type",
                MediaValueKind::Enum("TextDecorationType."),
            )
        };
        let color_value = if self.model.has_media("decoration-color") {
            self.media_binding("decoration-color", MediaValueKind::Color)
                .expect("checked decoration color")
        } else {
            self.media_default("decoration-color", MediaValueKind::Color)
        };
        let mut values = vec![("type".to_string(), type_value.into())];
        if color_value != "undefined" {
            values.push(("color".to_string(), color_value.into()));
        }
        self.properties.set("decoration", EtsValue::Object(values));
    }

    fn media_list_divider(&mut self) {
        let sources = [
            (
                "divider-stroke-width",
                "strokeWidth",
                MediaValueKind::String,
            ),
            ("divider-color", "color", MediaValueKind::Color),
            (
                "divider-start-margin",
                "startMargin",
                MediaValueKind::String,
            ),
            ("divider-end-margin", "endMargin", MediaValueKind::String),
        ];
        if !sources
            .iter()
            .any(|(source, _, _)| self.model.has_media(source))
        {
            return;
        }
        let mut values = Vec::new();
        for (source, target, kind) in sources {
            let mut value = self.media_default(source, kind);
            if value == "undefined" && source == "divider-stroke-width" {
                value = "0".to_string();
            }
            if self.model.has_media(source) {
                value = self
                    .media_binding(source, kind)
                    .expect("checked divider media");
            }
            values.push((target.to_string(), value.into()));
        }
        self.properties.set("divider", EtsValue::Object(values));
    }

    fn media_placeholder_font(&mut self) {
        let sources = [
            ("placeholderFont-size", "size", MediaValueKind::String),
            (
                "placeholderFont-weight",
                "weight",
                MediaValueKind::FontWeight,
            ),
            ("placeholderFont-family", "family", MediaValueKind::String),
            (
                "placeholderFont-style",
                "style",
                MediaValueKind::Enum("FontStyle."),
            ),
        ];
        if !sources
            .iter()
            .any(|(source, _, _)| self.model.has_media(source))
        {
            return;
        }
        let mut values = Vec::new();
        for (source, target, kind) in sources {
            let value = self
                .media_binding(source, kind)
                .unwrap_or_else(|| self.media_default(source, kind));
            values.push((target.to_string(), value.into()));
        }
        self.properties
            .set("placeholderFont", EtsValue::Object(values));
    }

    fn media_date_parameter(&mut self, source: &str, target: &str, decorator: &str) {
        self.media_parameter_with_decorator(source, target, MediaValueKind::Date, decorator);
    }

    fn media_parameter_with_decorator(
        &mut self,
        source: &str,
        target: &str,
        kind: MediaValueKind,
        decorator: &str,
    ) {
        if !self.model.has_media(source) {
            return;
        }
        let variable = self.register_media_variable(
            source,
            self.media_default(source, kind),
            Self::media_type(source),
            self.media_values(source, kind),
            decorator,
        );
        self.set_parameter(target, variable);
    }

    fn media_font_group(&mut self, target: &str, sources: &[(&str, &str)]) {
        if !sources
            .iter()
            .any(|(source, _)| self.model.has_media(source))
        {
            return;
        }
        let mut values = Vec::new();
        for (source, member) in sources {
            if !self.model.has_media(source) {
                continue;
            }
            let kind = match *member {
                "size" | "family" => MediaValueKind::String,
                "style" => MediaValueKind::Enum("FontStyle."),
                "weight" => MediaValueKind::FontWeight,
                _ => MediaValueKind::Raw,
            };
            let variable = self
                .media_binding(source, kind)
                .expect("checked font media");
            values.push((member.to_string(), variable.into()));
        }
        self.properties.set(target, EtsValue::Object(values));
    }

    fn media_search(&mut self) {
        let params = [
            ("search-value", "value"),
            ("search-placeholder", "placeholder"),
            ("icon", "icon"),
            ("search-controller", "controller"),
        ];
        let mut generated_params = Vec::new();
        for (source, target) in params {
            if !self.model.has_media(source) {
                continue;
            }
            let initial = if let Some(value) =
                VisualValue::optional_text(self.model.dynamic(source))
                    .filter(|value| !value.is_empty())
            {
                if target == "controller" {
                    value
                } else {
                    VisualValue::content_expression(&value)
                }
            } else {
                VisualValue::optional_text(self.model.property(source))
                    .filter(|value| !value.is_empty())
                    .map_or_else(|| "''".to_string(), |value| VisualValue::quote(&value))
            };
            let variable = self.register_media_variable(
                source,
                initial,
                Self::media_type(source),
                self.media_values(source, MediaValueKind::String),
                "",
            );
            generated_params.push(format!(
                "{target}: {}",
                VisualValue::content_expression(&variable)
            ));
        }
        if !generated_params.is_empty() {
            self.set_text_parameters(format!("{{{}}}", generated_params.join(", ")));
        }
        self.media_font_group(
            "textFont",
            &[
                ("text-font-size", "size"),
                ("text-font-style", "style"),
                ("text-font-weight", "weight"),
                ("text-font-family", "family"),
            ],
        );
        self.media_font_group(
            "placeholderFont",
            &[
                ("placeholder-font-size", "size"),
                ("placeholder-font-style", "style"),
                ("placeholder-font-weight", "weight"),
                ("placeholder-font-family", "family"),
            ],
        );
        self.media_property(
            "placeholderColor",
            "placeholderColor",
            MediaValueKind::Color,
        );
        self.media_property(
            "search-text-align",
            "textAlign",
            MediaValueKind::Enum("TextAlign."),
        );
        self.media_property("search-button", "searchButton", MediaValueKind::String);
        self.media_property(
            "copy-option",
            "copyOption",
            MediaValueKind::Enum("CopyOptions."),
        );
    }

    fn media_select_options(&mut self) {
        let source = "select-option";
        if !self.model.has_media(source) {
            return;
        }
        let initial = VisualValue::optional_text(self.model.dynamic(source))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "[{value: \"\"}]".to_string());
        let variable = self.register_media_variable(
            source,
            initial,
            Self::media_type(source),
            self.media_values(source, MediaValueKind::String),
            "",
        );
        self.set_text_parameters(variable);
    }

    fn media_tab_content(&mut self) {
        if !self.model.has_media("tabBar-icon") && !self.model.has_media("tabBar-text") {
            return;
        }
        let text = self
            .media_binding("tabBar-text", MediaValueKind::String)
            .unwrap_or_else(|| self.media_default("tabBar-text", MediaValueKind::String));
        let icon = self
            .media_binding("tabBar-icon", MediaValueKind::Source)
            .unwrap_or_else(|| self.media_default("tabBar-icon", MediaValueKind::Source));
        self.properties.set(
            "tabBar",
            EtsValue::Object(vec![
                ("text".to_string(), text.into()),
                ("icon".to_string(), icon.into()),
            ]),
        );
    }

    fn media_property(&mut self, source: &str, target: &str, kind: MediaValueKind) {
        if let Some(variable) = self.media_binding(source, kind) {
            self.properties.set(target, variable);
        }
    }

    fn media_parameter(&mut self, source: &str, target: &str, kind: MediaValueKind) {
        if let Some(variable) = self.media_binding(source, kind) {
            self.set_parameter(target, variable);
        }
    }

    fn media_text_parameters(&mut self, source: &str, kind: MediaValueKind) {
        if let Some(variable) = self.media_binding(source, kind) {
            self.set_text_parameters(variable);
        }
    }

    fn media_binding(&mut self, source: &str, kind: MediaValueKind) -> Option<String> {
        if !self.model.has_media(source) {
            return None;
        }
        let initial = self.media_default(source, kind);
        let value_type = Self::media_type(source);
        let assignments = self.media_values(source, kind);
        Some(self.register_media_variable(source, initial, value_type, assignments, ""))
    }

    fn media_values(&self, source: &str, kind: MediaValueKind) -> Vec<(String, String)> {
        let mut assignments = Vec::<(String, String)>::new();
        for (query, properties) in &self.model.media_properties.0 {
            if let Some(value) =
                VisualValue::optional_text(properties.get(source)).filter(|value| !value.is_empty())
            {
                assignments.push((query.clone(), kind.transform(&value)));
            }
        }
        for (query, properties) in &self.model.dynamic_media_properties.0 {
            if let Some(value) =
                VisualValue::optional_text(properties.get(source)).filter(|value| !value.is_empty())
            {
                let value = if matches!(kind, MediaValueKind::Content) {
                    VisualValue::content_expression(&value)
                } else {
                    value
                };
                Self::upsert_media_value(&mut assignments, query, value);
            }
        }
        assignments
    }

    fn media_queries(&self) -> Vec<String> {
        let mut queries = Vec::new();
        for (query, _) in self
            .model
            .media_properties
            .0
            .iter()
            .chain(&self.model.dynamic_media_properties.0)
        {
            if !queries.contains(query) {
                queries.push(query.clone());
            }
        }
        queries
    }

    fn upsert_media_value(values: &mut Vec<(String, String)>, query: &str, value: String) {
        if let Some((_, current)) = values.iter_mut().find(|(current, _)| current == query) {
            *current = value;
        } else {
            values.push((query.to_string(), value));
        }
    }

    fn register_media_variable(
        &mut self,
        source: &str,
        initial: String,
        value_type: &str,
        assignments: Vec<(String, String)>,
        decorator: &str,
    ) -> String {
        let variable = format!(
            "this.{}{}_{}",
            VisualValue::identifier(&self.model.id),
            VisualValue::upper_first(&VisualValue::identifier(source)),
            VisualValue::unique_id(&format!(
                "this.{}{}",
                VisualValue::identifier(&self.model.id),
                VisualValue::upper_first(&VisualValue::identifier(source))
            ))
        );
        self.defaults.push(VisualMediaDefault {
            variable: variable.clone(),
            value_type: value_type.to_string(),
            initial,
            decorator: decorator.to_string(),
        });
        self.media_assignments
            .extend(
                assignments
                    .into_iter()
                    .map(|(query, value)| VisualMediaAssignment {
                        query,
                        variable: variable.clone(),
                        value,
                    }),
            );
        variable
    }

    fn media_default(&self, source: &str, kind: MediaValueKind) -> String {
        let fallback = match source {
            "if" => "true",
            "for" => "[1]",
            "value" if self.model.tag_type == "progress" => "0",
            "auto-play" => "false",
            "interval" => "3000",
            "select-value" => "''",
            "columns-template" | "rows-template" | "tabBar-icon" => "''",
            "tabBar-text" => "'TabBar'",
            _ => "undefined",
        };
        if let Some(value) =
            VisualValue::optional_text(self.model.dynamic(source)).filter(|value| !value.is_empty())
        {
            return if matches!(kind, MediaValueKind::Content) {
                VisualValue::content_expression(&value)
            } else {
                value
            };
        }
        VisualValue::optional_text(self.model.property(source))
            .filter(|value| !value.is_empty())
            .map_or_else(|| fallback.to_string(), |value| kind.transform(&value))
    }

    fn media_type(source: &str) -> &'static str {
        match source {
            "width"
            | "height"
            | "constraint-size-min-width"
            | "constraint-size-max-width"
            | "constraint-size-min-height"
            | "constraint-size-max-height"
            | "left"
            | "top"
            | "border-width"
            | "border-top-width"
            | "border-bottom-width"
            | "border-left-width"
            | "border-right-width"
            | "border-radius"
            | "border-top-left-radius"
            | "border-bottom-right-radius"
            | "border-bottom-left-radius"
            | "border-top-right-radius"
            | "font-size"
            | "line-height"
            | "bar-width"
            | "bar-height"
            | "margin"
            | "margin-top"
            | "margin-bottom"
            | "margin-left"
            | "margin-right"
            | "padding"
            | "padding-top"
            | "padding-bottom"
            | "padding-left"
            | "padding-right"
            | "divider-stroke-width"
            | "divider-start-margin"
            | "divider-end-margin"
            | "placeholderFont-size"
            | "text-font-size"
            | "placeholder-font-size" => "string | number | Resource",
            "flex-basis" => "string | number",
            "direction" | "min" | "max" | "step" | "block-color" | "track-color"
            | "selected-color" | "show-steps" | "show-tips" => "",
            "align" | "align-content-stack" => "Alignment",
            "align-self" | "align-items-flex" => "ItemAlign",
            "border-style"
            | "border-top-style"
            | "border-bottom-style"
            | "border-left-style"
            | "border-right-style" => "BorderStyle",
            "border-color"
            | "background-color"
            | "font-color"
            | "color"
            | "placeholderColor"
            | "caretColor"
            | "selectedColor"
            | "switchPointColor"
            | "selected-option-font-color"
            | "selected-option-bg-color"
            | "option-font-color"
            | "option-bg-color"
            | "decoration-color"
            | "divider-color" => "string | number | Color | Resource",
            "border-top-color"
            | "border-bottom-color"
            | "border-left-color"
            | "border-right-color" => "string | number | Color | Resource",
            "background-image-repeat" | "object-repeat" => "ImageRepeat",
            "background-image-src" => "string | Resource",
            "background-image-size" => "ImageSize | string",
            "background-image-position" => "Alignment | string",
            "visibility" => "Visibility",
            "aspect-ratio"
            | "display-priority"
            | "flex-grow"
            | "flex-shrink"
            | "value"
            | "total"
            | "initial-index"
            | "index"
            | "interval"
            | "duration"
            | "cached-count"
            | "maxLength"
            | "selected"
            | "animation-duration"
            | "max-count"
            | "min-count"
            | "cell-length"
            | "row-start"
            | "row-end"
            | "column-start"
            | "column-end"
            | "source-size-width"
            | "source-size-height"
            | "textPicker-selected" => "number",
            "opacity" => "number | Resource",
            "if" => "boolean",
            "for" => "any[]",
            "enabled" | "state-effect" | "vertical" | "auto-play" | "indicator" | "loop"
            | "disable-swipe" | "edit-mode" | "chain-animation" | "editable"
            | "useMilitaryTime" | "refreshing" | "isOn" | "tabs-scrollable" | "hideToolBar"
            | "hideTitleBar" | "hideBackButton" | "multi-selectable" | "support-animation"
            | "force-rebuild" | "selectable" | "select" | "selectAll" => "boolean",
            "label" | "content" | "placeholder" | "alt" | "font-family" | "name" | "group"
            | "tabBar-text" => "string | Resource",
            "type" => "ButtonType",
            "font-style" => "FontStyle",
            "font-weight"
            | "placeholderFont-weight"
            | "text-font-weight"
            | "placeholder-font-weight"
            | "selected-option-font-weight"
            | "option-font-weight" => "FontWeight | number",
            "selected-option-font-size" | "option-font-size" => "string | number | Resource",
            "placeholderFont-style"
            | "text-font-style"
            | "placeholder-font-style"
            | "selected-option-font-style"
            | "option-font-style" => "FontStyle",
            "placeholderFont-family"
            | "text-font-family"
            | "placeholder-font-family"
            | "selected-option-font-family"
            | "option-font-family" => "string | Resource",
            "line-cap" => "LineCapStyle",
            "src" => "string | Resource",
            "object-fit" => "ImageFit",
            "interpolation" => "ImageInterpolation",
            "render-mode" => "ImageRenderMode",
            "style" => "ProgressStyle",
            "text-align" => "TextAlign",
            "search-text-align" => "TextAlign",
            "text-overflow" => "TextOverflow",
            "text-case" => "TextCase",
            "baseline-offset" => "string | number",
            "space"
            | "item-space"
            | "stroke-width"
            | "scroll-bar-width"
            | "friction"
            | "offset"
            | "defaultPickerItemHeight" => "string | number",
            "align-items-column" => "HorizontalAlign",
            "justify-content-rc" | "justify-content" | "align-content" => "FlexAlign",
            "align-items-row" => "VerticalAlign",
            "flex-direction" => "FlexDirection",
            "wrap" => "FlexWrap",
            "list-direction" => "Axis",
            "edge-effect" | "edge-effect-scroll" => "EdgeEffect",
            "sticky" => "Sticky",
            "textInput-type" => "InputType",
            "enterKeyType" => "EnterKeyType",
            "navigation-title" | "subTitle" | "select-value" | "columns-template"
            | "rows-template" => "string",
            "menus" | "range" | "select-option" => "any[]",
            "toolBar" => "object",
            "datetime-selected" | "start" | "end" => "Date",
            "lunar" => "boolean",
            "toggle-type" => "ToggleType",
            "scrollable" => "ScrollDirection",
            "scroll-bar" => "BarState",
            "scroll-bar-color" => "string | number | Color",
            "columns-gap" | "rows-gap" => "string | number | Resource",
            "layout-direction" => "GridDirection",
            "bar-position" => "BarPosition",
            "controller" => "TabsController",
            "bar-mode" => "BarMode",
            "tabBar-icon" => "string | Resource",
            "search-value" | "search-placeholder" | "icon" | "search-button" => "string",
            "search-controller" => "SearchController",
            "copy-option" => "CopyOption",
            "decoration-type" => "TextDecorationType",
            _ => "any",
        }
    }

    fn parse_events(&mut self) -> Result<(), VisualCodegenError> {
        const EVENTS: &[&str] = &[
            "onClick",
            "onTouch",
            "onAppear",
            "onDisappear",
            "onKeyEvent",
            "onAreaChange",
            "onItemDelete",
            "onScrollIndex",
            "onItemDragEnter",
            "onItemDragMove",
            "onItemDragLeave",
            "onItemDragStart",
            "onItemDrop",
            "onComplete",
            "onError",
            "onFinish",
            "onChange",
            "onSubmit",
            "onEditChanged",
            "onStateChange",
            "onRefreshing",
            "onSelect",
            "onScroll",
            "onScrollEdge",
            "onScrollEnd",
            "onScrollBegin",
            "onTitleModeChange",
            "onCopy",
            "onCut",
            "onPaste",
        ];
        for event in EVENTS {
            let Some(mut value) = VisualValue::optional_text(self.model.property(event)) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            if value.split('.').next() == Some("this") {
                value.push_str(".bind(this)");
            }
            self.properties.set(
                if *event == "onDisappear" {
                    "onDisAppear"
                } else {
                    event
                },
                value,
            );
        }
        if let Some(value) = VisualValue::optional_text(self.model.property("newFunction")) {
            let events: Value = serde_json::from_str(&value).map_err(|_| {
                VisualCodegenError::Codegen("invalid newFunction event object".to_string())
            })?;
            if let Some(events) = events.as_object() {
                for (event, value) in events {
                    let code = value
                        .get("eventCodeGen")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    self.properties.set(
                        if event == "onDisappear" {
                            "onDisAppear"
                        } else {
                            event
                        },
                        code.to_string(),
                    );
                }
            }
        }
        Ok(())
    }

    fn string_property(&mut self, source: &str, target: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        self.properties.set(
            target,
            if self.model.has_dynamic_value(source) {
                value
            } else {
                VisualValue::quote(&value)
            },
        );
    }

    fn raw_property(&mut self, source: &str, target: &str) {
        if let Some(value) = VisualValue::optional_text(self.model.effective(source))
            .filter(|value| !value.is_empty())
        {
            self.properties.set(target, value);
        }
    }

    fn enum_property(&mut self, source: &str, target: &str, prefix: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source))
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        self.properties.set(
            target,
            if self.model.has_dynamic_value(source) {
                value
            } else {
                VisualValue::enum_value(prefix, &value)
            },
        );
    }

    fn color_property(&mut self, source: &str, target: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source))
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        self.properties.set(
            target,
            if self.model.has_dynamic_value(source) {
                value
            } else {
                VisualValue::color(&value)
            },
        );
    }

    fn font_weight_property(&mut self, source: &str, target: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source))
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        self.properties.set(
            target,
            if self.model.has_dynamic_value(source) {
                value
            } else {
                VisualValue::font_weight(&value)
            },
        );
    }

    fn string_parameter(&mut self, source: &str, target: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source)) else {
            return;
        };
        self.set_parameter(
            target,
            if self.model.has_dynamic_value(source) {
                value
            } else {
                VisualValue::quote(&value)
            },
        );
    }

    fn raw_parameter(&mut self, source: &str, target: &str) {
        if let Some(value) = VisualValue::optional_text(self.model.effective(source))
            .filter(|value| !value.is_empty())
        {
            self.set_parameter(target, value);
        }
    }

    fn enum_parameter(&mut self, source: &str, target: &str, prefix: &str) {
        let Some(value) = VisualValue::optional_text(self.model.effective(source))
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        self.set_parameter(
            target,
            if self.model.has_dynamic_value(source) {
                value
            } else {
                VisualValue::enum_value(prefix, &value)
            },
        );
    }
}

#[derive(Clone, Copy)]
enum MediaValueKind {
    Raw,
    String,
    Source,
    Content,
    Color,
    FontWeight,
    ImageRepeat,
    Date,
    Enum(&'static str),
    HorizontalAlign,
    VerticalAlign,
    ItemAlign,
    FlexAlign,
    Wrap,
}

impl MediaValueKind {
    fn transform(self, value: &str) -> String {
        match self {
            Self::Raw => value.to_string(),
            Self::String => VisualValue::quote(value),
            Self::Source => VisualValue::source(value),
            Self::Content => VisualValue::quote(value),
            Self::Color => VisualValue::color(value),
            Self::FontWeight => VisualValue::font_weight(value),
            Self::ImageRepeat => match value {
                "repeat-x" => "ImageRepeat.X".to_string(),
                "repeat-y" => "ImageRepeat.Y".to_string(),
                "repeat" => "ImageRepeat.XY".to_string(),
                "no-repeat" => "ImageRepeat.NoRepeat".to_string(),
                _ => VisualValue::enum_value("ImageRepeat.", value),
            },
            Self::Date => {
                if value.is_empty() {
                    "new Date()".to_string()
                } else {
                    format!("new Date ({})", VisualValue::quote(value))
                }
            }
            Self::Enum(prefix) => VisualValue::enum_value(prefix, value),
            Self::HorizontalAlign => match value {
                "flex-start" => "HorizontalAlign.Start".to_string(),
                "flex-end" => "HorizontalAlign.End".to_string(),
                _ => VisualValue::enum_value("HorizontalAlign.", value),
            },
            Self::VerticalAlign => match value {
                "flex-start" => "VerticalAlign.Top".to_string(),
                "flex-end" => "VerticalAlign.Bottom".to_string(),
                _ => VisualValue::enum_value("VerticalAlign.", value),
            },
            Self::ItemAlign => match value {
                "flex-start" => "ItemAlign.Start".to_string(),
                "flex-end" => "ItemAlign.End".to_string(),
                _ => VisualValue::enum_value("ItemAlign.", value),
            },
            Self::FlexAlign => match value {
                "flex-start" => "FlexAlign.Start".to_string(),
                "flex-end" => "FlexAlign.End".to_string(),
                _ => VisualValue::enum_value("FlexAlign.", value),
            },
            Self::Wrap => {
                if value == "nowrap" {
                    "FlexWrap.NoWrap".to_string()
                } else {
                    VisualValue::enum_value("FlexWrap.", value)
                }
            }
        }
    }
}

struct VisualTagEmitter {
    cache: EtsCache,
    ets_import: EtsCache,
    ets_variable: EtsCache,
    ets_function: EtsCache,
    ets_builder: EtsCache,
    about_to_appear: EtsCache,
    imported_components: Vec<String>,
}

impl VisualTagEmitter {
    fn new() -> Self {
        Self {
            cache: EtsCache::new("  ", 2),
            ets_import: EtsCache::new("  ", 0),
            ets_variable: EtsCache::new("  ", 1),
            ets_function: EtsCache::new("  ", 1),
            ets_builder: EtsCache::new("  ", 1),
            about_to_appear: EtsCache::new("  ", 2),
            imported_components: Vec::new(),
        }
    }

    fn emit(mut self, tag: &VisualTag) -> VisualGeneratedSource {
        self.emit_media_query(tag);
        self.emit_tag(tag);
        let mut functions = self.ets_function.finish();
        functions.push_str(&self.ets_builder.finish());
        VisualGeneratedSource {
            build: self.cache.finish(),
            ets_import: self.ets_import.finish(),
            ets_variable: self.ets_variable.finish(),
            ets_function: functions,
            about_to_appear: self.about_to_appear.finish(),
        }
    }

    fn emit_media_query(&mut self, tag: &VisualTag) {
        let mut defaults = Vec::<VisualMediaDefault>::new();
        let mut queries = Vec::<(String, Vec<(String, String)>)>::new();
        Self::collect_media(tag, &mut defaults, &mut queries);
        if defaults.is_empty() && queries.is_empty() {
            return;
        }

        let media_query = Self::unique_name("mediaQuery");
        let initialize = Self::unique_name("initForMediaQuery");
        let callback = Self::unique_name("mediaQueryCb");
        let save_result = Self::unique_name("saveMediaQueryResult");
        let update = Self::unique_name("updateForMediaQuery");

        for default in &defaults {
            if !default.decorator.is_empty() {
                self.ets_variable.concat([default.decorator.as_str(), " "]);
                self.ets_variable.indent_off();
            }
            self.ets_variable.concat([
                default.variable.trim_start_matches("this."),
                ":",
                " ",
                &default.value_type,
                " ",
                "=",
                " ",
                "undefined",
                "\n",
            ]);
            self.ets_variable.indent_on();
        }
        self.ets_variable
            .concat([callback.as_str(), " ", "=", " ", "null", "\n"]);

        for (query, _) in &queries {
            let state = Self::query_state_name(query);
            self.ets_variable.concat([
                state.as_str(),
                ":",
                " ",
                "boolean",
                " ",
                "=",
                " ",
                "false",
                "\n",
            ]);
        }
        for (query, _) in &queries {
            let state = Self::query_state_name(query);
            self.ets_variable.concat([
                "listener_",
                state.as_str(),
                " ",
                "=",
                " ",
                media_query.as_str(),
                ".",
                "matchMediaSync",
                "(",
                "'",
                query.as_str(),
                "'",
                ")",
                "\n",
            ]);
        }

        let mut save = EtsCache::new("  ", 1);
        save.concat([
            save_result.as_str(),
            "(",
            "mediaQueryResult",
            ")",
            " ",
            "{",
            "\n",
        ]);
        save.inc_indent();
        save.concat([
            "switch",
            " ",
            "(",
            "mediaQueryResult.media",
            ")",
            " ",
            "{",
            "\n",
        ]);
        save.inc_indent();
        for (query, _) in &queries {
            let state = Self::query_state_name(query);
            save.concat(["case", " ", "'", query.as_str(), "'", ":", "\n"]);
            save.inc_indent();
            save.concat([
                "this",
                ".",
                state.as_str(),
                " ",
                "=",
                " ",
                "mediaQueryResult.matches",
                "\n",
            ]);
            save.concat(["break", "\n"]);
            save.dec_indent();
        }
        save.concat(["default", ":", "\n"]);
        save.inc_indent();
        save.concat(["break", "\n"]);
        save.dec_indent();
        save.dec_indent();
        save.concat(["}", "\n"]);
        save.concat(["this", ".", update.as_str(), "(", ")", "\n"]);
        save.dec_indent();
        save.concat(["}", "\n"]);

        let mut initialize_function = EtsCache::new("  ", 1);
        initialize_function.concat([initialize.as_str(), "(", ")", " ", "{", "\n"]);
        initialize_function.indent_on();
        initialize_function.inc_indent();
        for default in &defaults {
            initialize_function.concat([
                default.variable.as_str(),
                " ",
                "=",
                " ",
                default.initial.as_str(),
                "\n",
            ]);
        }
        initialize_function.dec_indent();
        initialize_function.concat(["}", "\n"]);

        let mut update_function = EtsCache::new("  ", 1);
        update_function.concat([update.as_str(), "(", ")", "{", "\n"]);
        update_function.inc_indent();
        update_function.concat(["this", ".", initialize.as_str(), "(", ")", "\n"]);
        for (query, assignments) in &queries {
            let state = Self::query_state_name(query);
            update_function.concat([
                "if",
                " ",
                "(",
                "this",
                ".",
                state.as_str(),
                ")",
                " ",
                "{",
                "\n",
            ]);
            update_function.inc_indent();
            for (variable, value) in assignments {
                update_function.concat([variable.as_str(), " ", "=", " ", value.as_str(), "\n"]);
            }
            update_function.dec_indent();
            update_function.concat(["}", "\n"]);
        }
        update_function.dec_indent();
        update_function.concat(["}", "\n"]);

        self.ets_import.indent_off();
        self.ets_variable.indent_off();
        self.ets_function.indent_off();
        self.ets_import.concat([
            "import ",
            media_query.as_str(),
            " from '@ohos.mediaquery'",
            "\n",
        ]);
        self.about_to_appear.concat([
            "this",
            ".",
            callback.as_str(),
            " ",
            "=",
            " ",
            "this",
            ".",
            save_result.as_str(),
            ".bind(this)",
            "\n",
        ]);
        for (index, (query, _)) in queries.iter().enumerate() {
            let state = Self::query_state_name(query);
            self.about_to_appear.indent_on();
            self.about_to_appear.concat([
                "this",
                ".",
                "listener_",
                state.as_str(),
                ".",
                "on('change', this.",
                callback.as_str(),
                ")",
            ]);
            if index + 1 < queries.len() {
                self.about_to_appear.indent_off();
                self.about_to_appear.concat(["\n"]);
            }
        }
        let save = save.finish();
        let initialize_function = initialize_function.finish();
        let update_function = update_function.finish();
        self.ets_function.concat([
            save.as_str(),
            "\n",
            initialize_function.as_str(),
            "\n",
            update_function.as_str(),
        ]);
        self.ets_import.indent_on();
        self.about_to_appear.indent_on();
        self.ets_variable.indent_on();
        self.ets_function.indent_on();
    }

    fn collect_media(
        tag: &VisualTag,
        defaults: &mut Vec<VisualMediaDefault>,
        queries: &mut Vec<(String, Vec<(String, String)>)>,
    ) {
        for default in &tag.defaults {
            if let Some(current) = defaults
                .iter_mut()
                .find(|current| current.variable == default.variable)
            {
                *current = default.clone();
            } else {
                defaults.push(default.clone());
            }
        }
        for assignment in &tag.media_assignments {
            let values = if let Some((_, values)) = queries
                .iter_mut()
                .find(|(query, _)| query == &assignment.query)
            {
                values
            } else {
                queries.push((assignment.query.clone(), Vec::new()));
                &mut queries.last_mut().expect("inserted query").1
            };
            if let Some((_, value)) = values
                .iter_mut()
                .find(|(variable, _)| variable == &assignment.variable)
            {
                *value = assignment.value.clone();
            } else {
                values.push((assignment.variable.clone(), assignment.value.clone()));
            }
        }
        for child in &tag.children {
            Self::collect_media(child, defaults, queries);
        }
    }

    fn unique_name(name: &str) -> String {
        format!("{name}_{}", VisualValue::unique_id(name))
    }

    fn query_state_name(query: &str) -> String {
        let device = query
            .split("device-type: ")
            .nth(1)
            .and_then(|value| value.split(')').next())
            .unwrap_or_default();
        let orientation = query
            .split("orientation: ")
            .nth(1)
            .and_then(|value| value.split(')').next())
            .unwrap_or_default();
        Self::unique_name(&format!(
            "{device}{}",
            VisualValue::upper_first(&orientation.to_ascii_lowercase())
        ))
    }

    fn emit_tag(&mut self, tag: &VisualTag) {
        if let Some((name, path)) = &tag.custom_import {
            if !self.imported_components.contains(name) {
                self.ets_import.concat([
                    "import ",
                    name.as_str(),
                    " from ",
                    "'",
                    path.as_str(),
                    "'",
                    "\n",
                ]);
                self.imported_components.push(name.clone());
            }
        }
        self.emit_start(tag);
        self.cache.concat([tag.tag_name.as_str(), "("]);
        self.cache.indent_off();
        self.emit_parameters(&tag.parameters);
        self.cache.concat([")"]);
        if !tag.children.is_empty() {
            self.cache.concat([" ", "{", "\n"]);
            self.cache.indent_on();
            self.cache.inc_indent();
            for child in &tag.children {
                self.emit_tag(child);
                self.cache.indent_off();
                self.cache.concat(["\n"]);
                self.cache.indent_on();
            }
            self.cache.dec_indent();
            self.cache.indent_on();
            self.cache.concat(["}"]);
        }
        if !tag.properties.0.is_empty() {
            if tag.children.is_empty() {
                self.cache.inc_indent();
            }
            for (name, value) in &tag.properties.0 {
                self.cache.concat(["\n"]);
                self.cache.indent_on();
                self.cache.concat([".", name, "("]);
                self.cache.indent_off();
                self.cache.concat([value.render().as_str(), ")"]);
            }
            if tag.children.is_empty() {
                self.cache.dec_indent();
            }
        }
        self.emit_end(tag);
        self.emit_builders(tag);
    }

    fn emit_parameters(&mut self, parameters: &TagParameters) {
        match parameters {
            TagParameters::Empty => {}
            TagParameters::Text(value) => self.cache.concat([value.as_str()]),
            TagParameters::Object(values) if values.is_empty() => {}
            TagParameters::Object(values) => {
                self.cache.concat(["{", " "]);
                for (index, (name, value)) in values.iter().enumerate() {
                    self.cache
                        .concat([name.as_str(), ":", " ", value.render().as_str()]);
                    if index + 1 < values.len() {
                        self.cache.concat([",", " "]);
                    }
                }
                self.cache.concat([" ", "}"]);
            }
            TagParameters::CustomObject(values) if values.is_empty() => {}
            TagParameters::CustomObject(values) => {
                self.cache.concat(["{"]);
                self.cache.inc_indent();
                self.cache.indent_on();
                for (name, value) in values {
                    self.cache.indent_off();
                    self.cache.concat(["\n"]);
                    self.cache.indent_on();
                    self.cache
                        .concat([name.as_str(), ":", " ", value.render().as_str(), ","]);
                }
                self.cache.indent_off();
                self.cache.concat(["\n"]);
                self.cache.indent_on();
                self.cache.dec_indent();
                self.cache.concat(["}"]);
                self.cache.indent_off();
            }
        }
    }

    fn emit_builders(&mut self, tag: &VisualTag) {
        for (name, body) in &tag.builders {
            let mut body_emitter = Self::new();
            body_emitter.cache = EtsCache::new("  ", 1);
            body_emitter.emit_tag(body);
            let body_source = body_emitter.cache.finish();
            let nested_builders = body_emitter.ets_builder.finish();

            self.ets_builder.indent_off();
            self.ets_builder.concat(["\n"]);
            self.ets_builder.indent_on();
            self.ets_builder
                .concat(["@Builder", " ", name.as_str(), "(", ")"]);
            self.ets_builder.indent_off();
            self.ets_builder.concat([" ", "{", "\n"]);
            self.ets_builder.indent_on();
            self.ets_builder.concat([body_source.as_str(), "\n"]);
            self.ets_builder.concat(["}", "\n"]);
            self.ets_builder.indent_off();
            self.ets_builder.concat([nested_builders.as_str()]);
        }
    }

    fn emit_start(&mut self, tag: &VisualTag) {
        if let Some(condition) = &tag.condition {
            self.cache
                .concat(["if", " ", "(", condition, ")", " ", "{", "\n"]);
            self.cache.inc_indent();
        }
        if let Some(for_each) = &tag.for_each {
            self.cache
                .concat(["ForEach", "(", &for_each.items, ",", "\n"]);
            self.cache.concat([
                "(",
                &for_each.item,
                ":",
                " any",
                ",",
                " ",
                &for_each.index,
                ":",
                " number",
                ")",
                " => ",
                "{",
                "\n",
            ]);
        }
    }

    fn emit_end(&mut self, tag: &VisualTag) {
        if let Some(for_each) = &tag.for_each {
            self.cache.concat(["}"]);
            if let Some(key) = &for_each.key {
                if !key.is_empty() {
                    self.cache.concat([",", "\n"]);
                    self.cache.indent_on();
                    self.cache.concat([
                        "(",
                        &for_each.item,
                        ":",
                        " any",
                        ",",
                        " ",
                        &for_each.index,
                        ":",
                        " number",
                        ")",
                        " => ",
                        "{",
                        "\n",
                    ]);
                    self.cache.concat([
                        "return ",
                        key,
                        "(",
                        &for_each.item,
                        ",",
                        " ",
                        &for_each.index,
                        ")",
                        "}",
                    ]);
                    self.cache.indent_off();
                }
            }
            self.cache.concat([")"]);
        }
        if tag.condition.is_some() {
            self.cache.indent_on();
            self.cache.dec_indent();
            self.cache.concat(["\n"]);
            self.cache.concat(["}"]);
            self.cache.indent_off();
        }
    }
}

struct EtsCache {
    value: String,
    indent_text: &'static str,
    indent: usize,
    indent_enabled: bool,
}

impl EtsCache {
    fn new(indent_text: &'static str, indent: usize) -> Self {
        Self {
            value: String::new(),
            indent_text,
            indent,
            indent_enabled: true,
        }
    }

    fn indent_on(&mut self) {
        self.indent_enabled = true;
    }

    fn indent_off(&mut self) {
        self.indent_enabled = false;
    }

    fn inc_indent(&mut self) {
        self.indent += 1;
    }

    fn dec_indent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    fn concat<'b>(&mut self, values: impl IntoIterator<Item = &'b str>) {
        if self.indent_enabled {
            for _ in 0..self.indent {
                self.value.push_str(self.indent_text);
            }
        }
        for value in values {
            self.value.push_str(value);
        }
    }

    fn finish(self) -> String {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct VisualFixture;

    impl VisualFixture {
        fn map(value: Value) -> Value {
            json!({ "dataType": "Map", "value": value })
        }

        fn model(
            tag_type: &str,
            id: &str,
            properties: Value,
            dynamic_properties: Value,
            children: Vec<Value>,
        ) -> Value {
            json!({
                "dataType": "VisualModel",
                "value": {
                    "id": id,
                    "type": tag_type,
                    "property": Self::map(properties),
                    "dynamicProperty": Self::map(dynamic_properties),
                    "children": children
                }
            })
        }

        fn model_with_media(
            tag_type: &str,
            id: &str,
            properties: Value,
            media_properties: Value,
        ) -> Value {
            json!({
                "dataType": "VisualModel",
                "value": {
                    "id": id,
                    "type": tag_type,
                    "property": Self::map(properties),
                    "dynamicProperty": Self::map(json!({})),
                    "mediaProperty": Self::map(media_properties),
                    "dynamicMediaProperty": Self::map(json!({})),
                    "children": []
                }
            })
        }

        fn source(model: Value) -> String {
            json!({
                "document": {
                    "VisualVersion": "12",
                    "type": "ETS",
                    "custom": false,
                    "path": "/project/entry/src/main/supervisual/pages/Index.visual"
                },
                "visualModel": model
            })
            .to_string()
        }
    }

    #[test]
    fn matches_sdk_nested_component_generation() {
        let text = VisualFixture::model(
            "text",
            "title",
            json!({
                "content": "Hello",
                "font-size": "16fp",
                "font-weight": "bold",
                "text-align": "center",
                "onClick": "this.tap"
            }),
            json!({}),
            vec![],
        );
        let column = VisualFixture::model(
            "column",
            "root",
            json!({
                "space": "8vp",
                "width": "100%",
                "height": "20vp",
                "padding": "4vp",
                "background-color": "red",
                "align-items-column": "center",
                "justify-content-rc": "space-between"
            }),
            json!({}),
            vec![text],
        );

        let generated = VisualCodeGenerator::new(&VisualFixture::source(column))
            .generate()
            .expect("generate Visual source");
        assert_eq!(
            generated.build,
            concat!(
                "    Column({ space: \"8vp\" }) {\n",
                "      Text(\"Hello\")\n",
                "        .textAlign(TextAlign.Center)\n",
                "        .fontSize(\"16fp\")\n",
                "        .fontWeight(FontWeight.Bold)\n",
                "        .onClick(this.tap.bind(this))\n",
                "    }    \n",
                "    .width(\"100%\")\n",
                "    .height(\"20vp\")\n",
                "    .backgroundColor(Color.Red)\n",
                "    .padding({ top: \"4vp\", bottom: \"4vp\", left: \"4vp\", right: \"4vp\" })\n",
                "    .alignItems(HorizontalAlign.Center)\n",
                "    .justifyContent(FlexAlign.SpaceBetween)"
            )
        );
    }

    #[test]
    fn matches_sdk_dynamic_if_and_for_each_generation() {
        let button = VisualFixture::model(
            "button",
            "button",
            json!({ "label": "Tap", "type": "capsule", "state-effect": "true" }),
            json!({}),
            vec![],
        );
        let row = VisualFixture::model(
            "row",
            "row",
            json!({ "space": "4vp", "margin": "1vp", "item": "item", "idx": "idx" }),
            json!({
                "width": "this.width",
                "if": "this.visible",
                "for": "this.items",
                "keyGenerator": "this.key"
            }),
            vec![button],
        );

        let generated = VisualCodeGenerator::new(&VisualFixture::source(row))
            .generate()
            .expect("generate Visual source");
        assert_eq!(
            generated.build,
            concat!(
                "    if (this.visible) {\n",
                "      ForEach(this.items,\n",
                "      (item: any, idx: number) => {\n",
                "      Row({ space: \"4vp\" }) {\n",
                "        Button(\"Tap\")\n",
                "          .type(ButtonType.Capsule)\n",
                "          .stateEffect(true)\n",
                "      }      \n",
                "      .width(this.width)\n",
                "      .margin({ top: \"1vp\", bottom: \"1vp\", left: \"1vp\", right: \"1vp\" })},\n",
                "      (item: any, idx: number) => {\n",
                "      return this.key(item, idx)})    \n",
                "    }"
            )
        );
    }

    #[test]
    fn validates_visual_file_and_version_like_sdk_parser() {
        assert_eq!(
            VisualCodeGenerator::new("not-json").generate(),
            Err(VisualCodegenError::File)
        );
        let source = json!({
            "document": { "VisualVersion": "13", "type": "ETS" },
            "visualModel": VisualFixture::model("column", "root", json!({}), json!({}), vec![])
        })
        .to_string();
        assert_eq!(
            VisualCodeGenerator::new(&source).generate(),
            Err(VisualCodegenError::Version)
        );
    }

    #[test]
    fn matches_sdk_media_query_fragments() {
        let query = "(device-type: phone) and (orientation: landscape)";
        let text = VisualFixture::model_with_media(
            "text",
            "title",
            json!({ "content": "Hello", "font-size": "12fp", "width": "100%" }),
            json!({ (query): VisualFixture::map(json!({ "font-size": "20fp", "width": "50%" })) }),
        );
        let generated = VisualCodeGenerator::new(&VisualFixture::source(text))
            .generate()
            .expect("generate media Visual source");

        assert_eq!(
            generated.build,
            concat!(
                "    Text(\"Hello\")\n",
                "      .width(this.titleWidth_aXRs)\n",
                "      .fontSize(this.titleFontSize_RsZU)"
            )
        );
        assert_eq!(
            generated.ets_import,
            "import mediaQuery_WFRd from '@ohos.mediaquery'\n"
        );
        assert_eq!(
            generated.ets_variable,
            concat!(
                "  titleWidth_aXRs: string | number | Resource = undefined\n",
                "  titleFontSize_RsZU: string | number | Resource = undefined\n",
                "  mediaQueryCb_FRdW = null\n",
                "  phoneLandscape_MYW5: boolean = false\n",
                "  listener_phoneLandscape_MYW5 = mediaQuery_WFRd.matchMediaSync('(device-type: phone) and (orientation: landscape)')\n"
            )
        );
        assert_eq!(
            generated.about_to_appear,
            concat!(
                "    this.mediaQueryCb_FRdW = this.saveMediaQueryResult_hUXV.bind(this)\n",
                "    this.listener_phoneLandscape_MYW5.on('change', this.mediaQueryCb_FRdW)"
            )
        );
        assert_eq!(
            generated.ets_function,
            concat!(
                "  saveMediaQueryResult_hUXV(mediaQueryResult) {\n",
                "    switch (mediaQueryResult.media) {\n",
                "      case '(device-type: phone) and (orientation: landscape)':\n",
                "        this.phoneLandscape_MYW5 = mediaQueryResult.matches\n",
                "        break\n",
                "      default:\n",
                "        break\n",
                "    }\n",
                "    this.updateForMediaQuery_yTWV()\n",
                "  }\n\n",
                "  initForMediaQuery_k1lZ() {\n",
                "    this.titleWidth_aXRs = \"100%\"\n",
                "    this.titleFontSize_RsZU = \"12fp\"\n",
                "  }\n\n",
                "  updateForMediaQuery_yTWV(){\n",
                "    this.initForMediaQuery_k1lZ()\n",
                "    if (this.phoneLandscape_MYW5) {\n",
                "      this.titleWidth_aXRs = \"50%\"\n",
                "      this.titleFontSize_RsZU = \"20fp\"\n",
                "    }\n",
                "  }\n"
            )
        );
    }

    #[test]
    fn matches_sdk_custom_component_import_params_and_builder_slot() {
        let component = "/project/entry/src/main/supervisual/components/Card";
        let attribute = format!("title-{}", component.replace('/', "-"));
        let slot = VisualFixture::model(
            "text",
            "slotText",
            json!({ "content": "Slot" }),
            json!({}),
            vec![],
        );
        let model = json!({
            "dataType": "VisualModel",
            "value": {
                "id": "card",
                "type": component,
                "property": VisualFixture::map(json!({
                    (attribute): "Hello",
                    "onTap": "this.tap"
                })),
                "dynamicProperty": VisualFixture::map(json!({})),
                "slots": VisualFixture::map(json!({ "content": slot })),
                "children": []
            }
        });
        let source = json!({
            "document": {
                "VisualVersion": "12",
                "type": "ETS",
                "custom": false,
                "path": "/project/entry/src/main/supervisual/pages/Index.visual"
            },
            "visualModel": model,
            "customData": {
                "property": VisualFixture::map(json!({
                    (component): VisualFixture::map(json!({
                        "title": { "type": "string", "decorator": "normal" }
                    }))
                })),
                "event": VisualFixture::map(json!({ (component): ["onTap"] }))
            }
        })
        .to_string();

        let generated = VisualCodeGenerator::new(&source)
            .generate()
            .expect("generate custom Visual source");
        assert_eq!(
            generated.build,
            concat!(
                "    Card({\n",
                "      title: \"Hello\",\n",
                "      onTap: this.tap.bind(this),\n",
                "    })\n",
                "      .content(this.cardContent_ENvb)"
            )
        );
        assert_eq!(
            generated.ets_import,
            "import Card from '../components/Card'\n"
        );
        assert_eq!(
            generated.ets_function,
            concat!(
                "\n",
                "  @Builder cardContent_ENvb() {\n",
                "    Text(\"Slot\")\n",
                "  }\n"
            )
        );
    }

    #[test]
    fn matches_sdk_custom_component_document_collision_and_package_paths() {
        let cases = [
            (
                "/project/entry/src/main/supervisual/pages/Index",
                "/project/entry/src/main/supervisual/components/Index",
                "Components_Index",
                "../components/Index",
            ),
            (
                "/project/entry/src/main/supervisual/pages/Index.visual",
                "/project/entry/src/main/supervisual/components/Index",
                "Index",
                "../components/Index",
            ),
            (
                "/project/entry/oh_modules/pkg/src/main/supervisual/pages/Host.visual",
                "/project/entry/oh_modules/pkg/src/main/supervisual/components/Card",
                "Card",
                "/project/entrypkg/src/main/ets/components/Card",
            ),
            (
                "/project/entry/oh_modules/@scope/pkg/src/main/supervisual/pages/Host.visual",
                "/project/entry/oh_modules/@scope/pkg/src/main/supervisual/components/Card",
                "Card",
                "/project/entry@@scope/pkg/src/main/ets/components/Card",
            ),
            (
                "/project/entry/node_modules/pkg/src/main/supervisual/pages/Host.visual",
                "/project/entry/node_modules/pkg/src/main/supervisual/components/Card",
                "Card",
                "../components/Card",
            ),
        ];

        for (document, component, tag_name, import_path) in cases {
            let source = json!({
                "document": {
                    "VisualVersion": "12",
                    "type": "ETS",
                    "custom": false,
                    "path": document
                },
                "visualModel": VisualFixture::model(
                    component,
                    "custom",
                    json!({}),
                    json!({}),
                    vec![]
                )
            })
            .to_string();
            let generated = VisualCodeGenerator::new(&source)
                .generate()
                .unwrap_or_else(|error| panic!("{document}: {error}"));
            assert_eq!(generated.build, format!("    {tag_name}()"), "{document}");
            assert_eq!(
                generated.ets_import,
                format!("import {tag_name} from '{import_path}'\n"),
                "{document}"
            );
        }
    }

    #[test]
    fn upgrades_visual_version_one_documents_like_sdk_deserializer() {
        let source = json!({
            "VisualVersion": "1",
            "content": json!({
                "id": "root",
                "tagName": "column",
                "attributes": [],
                "idStyle": [],
                "type": "Base",
                "content": [{
                    "id": "title",
                    "tagName": "text",
                    "attributes": [],
                    "idStyle": [],
                    "type": "Base",
                    "content": "Hello"
                }]
            }).to_string(),
            "extraData": json!(["$$map", ["textMap", ["$$map"]]]).to_string()
        })
        .to_string();

        let generated = VisualCodeGenerator::new(&source)
            .generate()
            .expect("generate legacy Visual source");
        assert_eq!(
            generated.build,
            concat!("    Column() {\n", "      Text(\"Hello\")\n", "    }")
        );
    }

    #[test]
    fn matches_sdk_compound_media_component_generation() {
        let query = "(device-type: phone) and (orientation: landscape)";
        let cases = [
            (
                "common",
                VisualFixture::model_with_media(
                    "column",
                    "root",
                    json!({
                        "left": "1vp",
                        "top": "2vp",
                        "position": "relative",
                        "border-width": "1vp",
                        "border-color": "red",
                        "background-image-size": "contain",
                        "margin": "3vp"
                    }),
                    json!({ (query): VisualFixture::map(json!({
                        "constraint-size-min-width": "10vp",
                        "left": "4vp",
                        "top": "5vp",
                        "position": "absolute",
                        "border-top-width": "2vp",
                        "border-left-color": "blue",
                        "background-image-size": "20vp 30vp",
                        "margin-left": "6vp"
                    })) }),
                ),
                concat!(
                    "    Column()\n",
                    "      .offset({ x: this.rootOffsetLeft_0T2Z, y: this.rootOffsetTop_90T2 })\n",
                    "      .borderWidth({ top: this.rootBorderTopWidth_m9yZ, bottom: this.rootBorderBottomWidth_yZGV, left: this.rootBorderLeftWidth_9yZG, right: this.rootBorderRightWidth_yZGV })\n",
                    "      .borderColor({ top: this.rootBorderTopColor_m9yZ, bottom: this.rootBorderBottomColor_yZGV, left: this.rootBorderLeftColor_9yZG, right: this.rootBorderRightColor_yZGV })\n",
                    "      .backgroundImageSize(this.rootBackgroundImageSizeEnum_yb3V !== undefined ? this.rootBackgroundImageSizeEnum_yb3V : this.rootBackgroundImageSizeObj_yb3V)\n",
                    "      .margin({ top: \"3vp\", bottom: \"3vp\", left: this.rootMarginLeft_0TWF, right: \"3vp\" })\n",
                    "      .constraintSize({ minWidth: this.rootConstraintSizeMinWidth_yYWl })\n",
                    "      .position({ x: this.rootPositionLeft_UG9z, y: this.rootPositionTop_0UG9 })"
                ),
            ),
            (
                "text",
                VisualFixture::model_with_media(
                    "text",
                    "title",
                    json!({
                        "content": "Hello",
                        "text-overflow": "clip",
                        "decoration-type": "underline",
                        "decoration-color": "red"
                    }),
                    json!({ (query): VisualFixture::map(json!({
                        "text-overflow": "ellipsis",
                        "decoration-color": "blue"
                    })) }),
                ),
                concat!(
                    "    Text(\"Hello\")\n",
                    "      .textOverflow({ overflow: this.titleTextOverflow_VRle })\n",
                    "      .decoration({ type: TextDecorationType.Underline, color: this.titleDecorationColor_lY29 })"
                ),
            ),
            (
                "image",
                VisualFixture::model_with_media(
                    "image",
                    "image",
                    json!({
                        "src": "logo",
                        "source-size-width": "10vp",
                        "source-size-height": "20vp"
                    }),
                    json!({ (query): VisualFixture::map(json!({
                        "source-size-width": "30vp"
                    })) }),
                ),
                concat!(
                    "    Image(\"logo\")\n",
                    "      .sourceSize({ width: this.imageSourceSizeWidth_vdXJ, height: this.imageSourceSizeHeight_vdXJ })"
                ),
            ),
            (
                "list",
                VisualFixture::model_with_media(
                    "list",
                    "list",
                    json!({}),
                    json!({ (query): VisualFixture::map(json!({
                        "divider-color": "blue",
                        "divider-end-margin": "2vp"
                    })) }),
                ),
                concat!(
                    "    List()\n",
                    "      .divider({ strokeWidth: 0, color: this.listDividerColor_RGl2, startMargin: undefined, endMargin: this.listDividerEndMargin_2aWR })"
                ),
            ),
            (
                "text input",
                VisualFixture::model_with_media(
                    "text-input",
                    "input",
                    json!({
                        "placeholderFont-size": "12fp",
                        "placeholderFont-weight": "bold"
                    }),
                    json!({ (query): VisualFixture::map(json!({
                        "placeholderFont-style": "italic"
                    })) }),
                ),
                concat!(
                    "    TextInput()\n",
                    "      .placeholderFont({ size: \"12fp\", weight: FontWeight.Bold, family: undefined, style: this.inputPlaceholderFontStyle_NlaG })"
                ),
            ),
            (
                "date picker",
                VisualFixture::model_with_media(
                    "datePicker",
                    "picker",
                    json!({ "datetime-selected": "2024-01-01" }),
                    json!({ (query): VisualFixture::map(json!({
                        "start": "2020-01-01",
                        "lunar": true
                    })) }),
                ),
                concat!(
                    "    DatePicker({ selected: new Date (\"2024-01-01\"), start: this.pickerStart_WNrZ })\n",
                    "      .lunar(this.pickerLunar_WNrZ)"
                ),
            ),
            (
                "select",
                VisualFixture::model_with_media(
                    "select",
                    "select",
                    json!({
                        "select-option": "",
                        "font-size": "12fp",
                        "font-family": "sans"
                    }),
                    json!({ (query): VisualFixture::map(json!({
                        "font-size": "14fp",
                        "font-weight": "bold",
                        "option-font-style": "italic"
                    })) }),
                ),
                concat!(
                    "    Select([{value: \"\"}])\n",
                    "      .font({ size: this.selectFontSize_lY3R, weight: this.selectFontWeight_Y3RG })\n",
                    "      .optionFont({ style: this.selectOptionFontStyle_PcHR })"
                ),
            ),
            (
                "search",
                VisualFixture::model_with_media(
                    "search",
                    "search",
                    json!({ "search-value": "Hello", "text-font-size": "12fp" }),
                    json!({ (query): VisualFixture::map(json!({
                        "search-placeholder": "Find",
                        "text-font-weight": "bold",
                        "copy-option": "local"
                    })) }),
                ),
                concat!(
                    "    Search({placeholder: `${this.searchSearchPlaceholder_WFyY}`})\n",
                    "      .textFont({ weight: this.searchTextFontWeight_UZXh })\n",
                    "      .copyOption(this.searchCopyOption_Y2hD)"
                ),
            ),
            (
                "tab content",
                VisualFixture::model_with_media(
                    "tab-content",
                    "tab",
                    json!({ "tabBar-text": "Base" }),
                    json!({ (query): VisualFixture::map(json!({
                        "tabBar-icon": "$r('app.media.icon')"
                    })) }),
                ),
                concat!(
                    "    TabContent()\n",
                    "      .tabBar({ text: \"Base\", icon: this.tabTabBarIcon_JUYW })"
                ),
            ),
        ];

        for (name, model, expected) in cases {
            let generated = VisualCodeGenerator::new(&VisualFixture::source(model))
                .generate()
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(generated.build, expected, "{name}");
            match name {
                "common" => {
                    assert!(generated.ets_function.contains(
                        "this.rootOffsetLeft_0T2Z = undefined\n      this.rootOffsetTop_90T2 = undefined"
                    ));
                    assert!(generated
                        .ets_function
                        .contains("this.rootBackgroundImageSizeEnum_yb3V = ImageSize.Contain"));
                    assert!(generated.ets_function.contains(
                        "this.rootBackgroundImageSizeObj_yb3V = { width: \"20vp\", height: \"30vp\" }"
                    ));
                }
                "date picker" => assert!(generated
                    .ets_variable
                    .contains("private pickerStart_WNrZ: Date = undefined")),
                "search" => {
                    assert!(generated
                        .ets_function
                        .contains("this.searchSearchPlaceholder_WFyY = ''"));
                    assert!(generated
                        .ets_function
                        .contains("this.searchSearchPlaceholder_WFyY = \"Find\""));
                }
                _ => {}
            }
        }
    }
}
