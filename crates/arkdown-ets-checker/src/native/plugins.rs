use super::{NativeOhSdkPluginHost, NativeOhSdkPluginRequest};
use std::sync::{Arc, OnceLock};
use tsrs_api::embedded::{SdkPluginHost, SdkPluginRequest};
use tsrs_core::OhSdkPluginResponse;

pub(super) struct PluginHost {
    host: Arc<dyn NativeOhSdkPluginHost>,
    session: OnceLock<u64>,
}
impl PluginHost {
    pub(super) fn new(host: Arc<dyn NativeOhSdkPluginHost>) -> Self {
        Self {
            host,
            session: OnceLock::new(),
        }
    }
}
impl Drop for PluginHost {
    fn drop(&mut self) {
        let Some(&session_id) = self.session.get() else {
            return;
        };
        // Compiler options/snapshot leases retain this host. Its last drop is
        // the point when no SDK query can still refer to this session's ASTs.
        if let Err(error) = self.host.invoke_batch(vec![NativeOhSdkPluginRequest {
            session_id,
            path: Default::default(),
            function_name: String::new(),
            class_name: String::new(),
            operation: "releaseSession".into(),
            args: Vec::new(),
        }]) {
            tracing::warn!(%error, session_id, "SDK callback session cleanup failed");
        } else {
            tracing::info!(session_id, "SDK callback session released");
        }
    }
}
impl SdkPluginHost for PluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<SdkPluginRequest>,
    ) -> Result<Vec<OhSdkPluginResponse>, String> {
        if let Some(request) = requests.first() {
            let session_id = *self.session.get_or_init(|| request.session_id);
            if requests
                .iter()
                .any(|request| request.session_id != session_id)
            {
                return Err("SDK callback batch belongs to a different compiler session".into());
            }
        }
        self.host
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
