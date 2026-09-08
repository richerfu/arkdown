//! OH TypeScript `types.ts::EtsOptions`, supplied by ets-loader tsconfig.json.
//! Preserve absent/null versus explicit empty tables: Builder fallback depends
//! on this distinction. These are data, not a registry of guessed defaults.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct EtsOptions {
    pub render: EtsRender,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Box<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libs: Option<Vec<Box<str>>>,
    pub extend: EtsExtend,
    pub styles: EtsStyles,
    pub concurrent: EtsConcurrent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_component: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_decorators: Option<Vec<EtsPropertyDecorator>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emit_decorators: Option<Vec<EtsEmitDecorator>>,
    pub syntax_components: EtsSyntaxComponents,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EtsRender {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<Vec<Box<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorator: Option<Vec<Box<str>>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EtsExtend {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorator: Option<Vec<Box<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<EtsComponent>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EtsComponent {
    pub name: Box<str>,
    #[serde(rename = "type")]
    pub type_name: Box<str>,
    pub instance: Box<str>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EtsStyles {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorator: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<EtsComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<Box<str>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EtsConcurrent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorator: Option<Box<str>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EtsPropertyDecorator {
    pub name: Box<str>,
    #[serde(default)]
    pub need_initialization: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EtsEmitDecorator {
    pub name: Box<str>,
    #[serde(default)]
    pub emit_parameters: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct EtsSyntaxComponents {
    #[serde(rename = "paramsUICallback", skip_serializing_if = "Option::is_none")]
    pub params_ui_callback: Option<Vec<Box<str>>>,
    #[serde(rename = "attrUICallback", skip_serializing_if = "Option::is_none")]
    pub attr_ui_callback: Option<Vec<EtsAttributeCallback>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EtsAttributeCallback {
    pub name: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<Box<str>>>,
}
