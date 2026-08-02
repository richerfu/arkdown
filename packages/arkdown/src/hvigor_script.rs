use napi::bindgen_prelude::{block_on, Error, Result, Status};

use arkdown::HvigorScriptCompiler;

pub(crate) struct HvigorScriptNapiCompiler;

impl HvigorScriptNapiCompiler {
  pub(crate) async fn compile(file: String, source_map: bool) -> Result<String> {
    HvigorScriptCompiler::new(file)
      .with_source_map(source_map)
      .compile()
      .await
      .map_err(|error| {
        Error::new(
          Status::GenericFailure,
          format!("HVIGOR_TYPESCRIPT_DIAGNOSTIC: {error}"),
        )
      })
  }

  pub(crate) fn compile_sync(file: String, source_map: bool) -> Result<String> {
    block_on(Self::compile(file, source_map))
  }
}
