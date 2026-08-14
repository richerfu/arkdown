use napi::{Error, Result, Status};

use arkdown_session::{RemoteHspObjectInventory, RemoteHspObjectRequest};

pub(crate) struct RemoteHspNapiAdapter;

impl RemoteHspNapiAdapter {
  pub(crate) fn inspect(request_json: &str) -> Result<String> {
    let request =
      serde_json::from_str::<RemoteHspObjectRequest>(request_json).map_err(|error| {
        Error::new(
          Status::InvalidArg,
          format!("ARKDOWN_INVALID_REMOTE_HSP_REQUEST: {error}"),
        )
      })?;
    let response = match RemoteHspObjectInventory::new(request).inspect() {
      Ok(objects) => serde_json::json!({ "objects": objects }),
      Err(error) => serde_json::json!({
        "error": {
          "code": error.code(),
          "message": error.to_string(),
          "path": error.path(),
          "version": error.version(),
        }
      }),
    };
    serde_json::to_string(&response).map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("ARKDOWN_REMOTE_HSP_RESPONSE_SERIALIZE_FAILED: {error}"),
      )
    })
  }
}
