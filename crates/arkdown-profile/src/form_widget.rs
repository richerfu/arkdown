use arkdown_core::ModuleJson5;
use arkdown_session::ProjectSnapshot;

/// Derives the form abilities contributed by the paired extension module from
/// immutable project-session data.
pub struct FormWidgetAbilityMerger<'a> {
    project: &'a ProjectSnapshot,
}

impl<'a> FormWidgetAbilityMerger<'a> {
    pub fn new(project: &'a ProjectSnapshot) -> Self {
        Self { project }
    }

    pub fn collect(&self, module_json: &ModuleJson5) -> Vec<serde_json::Value> {
        let Some(extension_module_name) = module_json.module.form_extension_module.as_deref()
        else {
            return Vec::new();
        };
        let Some(extension_json) = self
            .project
            .modules
            .get(extension_module_name)
            .and_then(|module| module.module_json.as_ref())
        else {
            return Vec::new();
        };

        extension_json
            .module
            .extension_abilities
            .iter()
            .filter(|ability| ability.extension_type == "form")
            .map(|ability| {
                serde_json::json!({
                    "name": ability.name,
                    "type": "form",
                    "metadata": ability.metadata.iter().map(|entry| {
                        let mut value = serde_json::Map::new();
                        value.insert("name".into(), entry.name.clone().into());
                        if let Some(resource) = entry.resource.as_ref() {
                            value.insert("resource".into(), resource.clone().into());
                        }
                        serde_json::Value::Object(value)
                    }).collect::<Vec<_>>(),
                })
            })
            .collect()
    }
}
