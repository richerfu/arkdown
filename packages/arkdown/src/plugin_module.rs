use napi::bindgen_prelude::{block_on, Error, Result, Status};

use arkdown::PluginModuleCompiler;

pub(crate) struct PluginModuleNapiCompiler;

impl PluginModuleNapiCompiler {
  pub(crate) async fn compile(file: String, source_map: bool) -> Result<String> {
    PluginModuleCompiler::new(file)
      .with_source_map(source_map)
      .compile()
      .await
      .map_err(|error| {
        Error::new(
          Status::GenericFailure,
          format!("ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR: {error}"),
        )
      })
  }

  pub(crate) fn compile_sync(file: String, source_map: bool) -> Result<String> {
    block_on(Self::compile(file, source_map))
  }
}
