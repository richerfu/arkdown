use rustc_hash::FxHashMap;

pub(crate) type HvigorChunkImportMap = FxHashMap<String, String>;

pub(crate) struct HvigorChunkModuleRequestProcessor {
    bundle_name: String,
    module_name: String,
}

impl HvigorChunkModuleRequestProcessor {
    pub(crate) fn new(bundle_name: impl Into<String>, module_name: impl Into<String>) -> Self {
        Self {
            bundle_name: bundle_name.into(),
            module_name: module_name.into(),
        }
    }

    pub(crate) fn process_request(&self, request: &str) -> String {
        HvigorModuleRequest::new(request, &self.bundle_name, &self.module_name)
            .to_ohm_request()
            .unwrap_or_else(|| request.to_string())
    }
}

struct HvigorModuleRequest<'a> {
    request: &'a str,
    bundle_name: &'a str,
    module_name: &'a str,
}

impl<'a> HvigorModuleRequest<'a> {
    const NATIVE_MODULES: &'static [&'static str] = &[
        "system.app",
        "ohos.app",
        "system.router",
        "system.curves",
        "ohos.curves",
        "system.matrix4",
        "ohos.matrix4",
    ];

    fn new(request: &'a str, bundle_name: &'a str, module_name: &'a str) -> Self {
        Self {
            request,
            bundle_name,
            module_name,
        }
    }

    fn to_ohm_request(&self) -> Option<String> {
        if let Some(so_name) = self
            .request
            .strip_prefix("lib")
            .and_then(|request| request.strip_suffix(".so"))
            .filter(|name| !name.is_empty())
        {
            return Some(format!(
                "@app:{}/{}/{}",
                self.bundle_name, self.module_name, so_name
            ));
        }

        let request = self.request.strip_prefix('@')?;
        if let Some((prefix, module)) = request.split_once(':') {
            if module.is_empty() {
                return None;
            }
            if let Some(native_module) = Self::native_module_request(prefix, module) {
                return Some(native_module);
            }
            return match prefix {
                "ohos" => Some(format!("@ohos:{module}")),
                "hms" => Some(format!("@hms:{module}")),
                "native" => Some(format!("@native:{module}")),
                _ => None,
            };
        }

        let (prefix, module) = request.split_once('.')?;
        if module.is_empty() {
            return None;
        }
        if let Some(native_module) = Self::native_module_request(prefix, module) {
            return Some(native_module);
        }
        match prefix {
            "ohos" => Some(format!("@ohos:{module}")),
            "hms" => Some(format!("@hms:{module}")),
            "kit" => Some(format!("@ohos:{module}")),
            "arkts" => Some(format!("@ohos:arkts.{module}")),
            "native" => Some(format!("@native:{module}")),
            "system" => Some(format!("@ohos:{module}")),
            _ => None,
        }
    }

    fn native_module_request(prefix: &str, module: &str) -> Option<String> {
        let full_name = if prefix == "ohos" && module.starts_with("system.") {
            module.to_string()
        } else {
            [prefix, module].join(".")
        };
        Self::NATIVE_MODULES
            .contains(&full_name.as_str())
            .then(|| ["@native:", full_name.as_str()].concat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn processor() -> HvigorChunkModuleRequestProcessor {
        HvigorChunkModuleRequestProcessor::new("com.example.hmosworld", "phone")
    }

    #[test]
    fn rewrites_system_api_requests() {
        let processor = processor();

        assert_eq!(
            processor.process_request("@ohos.data.preferences"),
            "@ohos:data.preferences"
        );
        assert_eq!(
            processor.process_request("@ohos.net.connection"),
            "@ohos:net.connection"
        );
        assert_eq!(
            processor.process_request("@hms.core.map.map"),
            "@hms:core.map.map"
        );
        assert_eq!(
            processor.process_request("@system.router"),
            "@native:system.router"
        );
    }

    #[test]
    fn rewrites_sdk_native_modules_to_hvigor_native_requests() {
        let processor = processor();

        assert_eq!(
            processor.process_request("@ohos:system.app"),
            "@native:system.app"
        );
        assert_eq!(
            processor.process_request("@system.app"),
            "@native:system.app"
        );
        assert_eq!(
            processor.process_request("@ohos.curves"),
            "@native:ohos.curves"
        );
        assert_eq!(
            processor.process_request("@system.promptAction"),
            "@ohos:promptAction"
        );
    }

    #[test]
    fn rewrites_arkts_sdk_requests_like_hvigor() {
        let processor = processor();

        assert_eq!(
            processor.process_request("@arkts.utils"),
            "@ohos:arkts.utils"
        );
        assert_eq!(
            processor.process_request("@arkts.collections"),
            "@ohos:arkts.collections"
        );
        assert_eq!(processor.process_request("@kit.ArkTS"), "@ohos:ArkTS");
    }

    #[test]
    fn rewrites_lib_so_requests_to_app_ohm_url() {
        let processor = processor();

        assert_eq!(
            processor.process_request("libagccrypto.so"),
            "@app:com.example.hmosworld/phone/agccrypto"
        );
        assert_eq!(
            processor.process_request("libagcmbedtls.so"),
            "@app:com.example.hmosworld/phone/agcmbedtls"
        );
    }

    #[test]
    fn leaves_unmatched_requests_unchanged() {
        let processor = processor();

        assert_eq!(processor.process_request("@ohos/hypium"), "@ohos/hypium");
        assert_eq!(processor.process_request("./local"), "./local");
    }
}
