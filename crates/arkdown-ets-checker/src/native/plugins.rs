use super::{NativeOhSdkPluginHost, NativeOhSdkPluginRequest};
use std::sync::{Arc, OnceLock};
use tsrs_api::embedded::{SdkPluginCompletion, SdkPluginHost, SdkPluginRequest};
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

    pub(super) fn compact_session(&self) {
        self.submit_session_operation("compactSession");
    }

    pub(super) fn release_sources(&self) {
        self.submit_session_operation("releaseSources");
    }

    fn submit_session_operation(&self, operation: &'static str) {
        let Some(&session_id) = self.session.get() else {
            return;
        };
        // Enqueue at the owning compiler boundary without waiting on JS.
        self.host.submit_batch(
            vec![NativeOhSdkPluginRequest {
                session_id,
                plugin_id: 0,
                path: None,
                function_name: None,
                class_name: None,
                operation: operation.into(),
                after: None,
                args: Vec::new(),
            }],
            Box::new(move |result| match result {
                Ok(responses) => match responses.as_slice() {
                    [response] if response.error.is_empty() => {
                        tracing::info!(session_id, operation, stats = ?response.stats, "{}",
                            if operation == "compactSession" { "SDK callback session compacted" }
                            else { "SDK callback session sources released" });
                    }
                    [response] => tracing::warn!(
                        session_id, operation, error = %response.error, "SDK callback session operation failed"
                    ),
                    _ => tracing::warn!(
                        session_id, operation, responses = responses.len(), "Invalid SDK session operation response count"
                    ),
                },
                Err(error) => tracing::warn!(%error, session_id, operation, "SDK callback session operation failed"),
            }),
        );
    }
}
impl Drop for PluginHost {
    fn drop(&mut self) {
        let Some(&session_id) = self.session.get() else {
            return;
        };
        // Compiler options/snapshot leases retain this host. Its last drop is
        // the point when no SDK query can still refer to this session's ASTs.
        match self.host.invoke_batch(vec![NativeOhSdkPluginRequest {
            session_id,
            plugin_id: 0,
            path: None,
            function_name: None,
            class_name: None,
            operation: "releaseSession".into(),
            after: None,
            args: Vec::new(),
        }]) {
            Err(error) => tracing::warn!(%error, session_id, "SDK callback session cleanup failed"),
            Ok(responses) => {
                let stats = responses
                    .first()
                    .and_then(|response| response.stats.as_ref());
                tracing::info!(session_id, ?stats, "SDK callback session released");
            }
        }
    }
}
impl PluginHost {
    fn requests(
        &self,
        requests: Vec<SdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginRequest>, String> {
        if let Some(request) = requests.first() {
            let session_id = *self.session.get_or_init(|| request.session_id);
            if requests
                .iter()
                .any(|request| request.session_id != session_id)
            {
                return Err("SDK callback batch belongs to a different compiler session".into());
            }
        }
        Ok(requests
            .into_iter()
            .map(|r| NativeOhSdkPluginRequest {
                session_id: r.session_id,
                plugin_id: r.plugin_id,
                path: r.path.map(Into::into),
                function_name: r.function_name,
                class_name: r.class_name,
                operation: r.operation,
                after: r.after,
                args: r.args,
            })
            .collect())
    }
    fn responses(
        responses: Result<Vec<super::NativeOhSdkPluginResponse>, super::NativeCheckError>,
    ) -> Result<Vec<OhSdkPluginResponse>, String> {
        responses
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
                        skipped: r.skipped,
                    })
                    .collect()
            })
            .map_err(|e| e.to_string())
    }
}
impl SdkPluginHost for PluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<SdkPluginRequest>,
    ) -> Result<Vec<OhSdkPluginResponse>, String> {
        Self::responses(self.host.invoke_batch(self.requests(requests)?))
    }
    fn submit_batch(&self, requests: Vec<SdkPluginRequest>, complete: SdkPluginCompletion) {
        match self.requests(requests) {
            Ok(requests) => self.host.submit_batch(
                requests,
                Box::new(move |responses| complete(Self::responses(responses))),
            ),
            Err(error) => complete(Err(error)),
        }
    }
}
