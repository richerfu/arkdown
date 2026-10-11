use super::{NativeOhSdkPluginHost, NativeOhSdkPluginRequest};
use std::sync::Arc;
use tsrs_api::embedded::{SdkPluginHost, SdkPluginRequest};
use tsrs_core::OhSdkPluginResponse;

pub(super) struct PluginHost(pub(super) Arc<dyn NativeOhSdkPluginHost>);
impl SdkPluginHost for PluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<SdkPluginRequest>,
    ) -> Result<Vec<OhSdkPluginResponse>, String> {
        self.0
            .invoke_batch(
                requests
                    .into_iter()
                    .map(|r| NativeOhSdkPluginRequest {
                        session_id: r.session_id,
                        path: r.path.into(),
                        function_name: r.function_name,
                        class_name: r.class_name,
                        operation: r.operation,
                        args: r.args,
                    })
                    .collect(),
            )
            .map(|responses| {
                responses
                    .into_iter()
                    .map(|r| OhSdkPluginResponse {
                        found: r.found,
                        result: r.result,
                        message: r.message,
                        valid: r.valid,
                        version: r.version,
                        matched: r.matched,
                        groups: r.groups,
                        phase: r.phase,
                        error: r.error,
                        check_result: r.check_result,
                        check_message: r.check_message,
                    })
                    .collect()
            })
            .map_err(|e| e.to_string())
    }
}
