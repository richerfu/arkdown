use napi::{Error, Result, Status};

use arkdown::HvigorJsonParser;

pub(crate) struct HvigorJsonNapiParser;

impl HvigorJsonNapiParser {
  pub(crate) fn parse(source: &str, locations: bool) -> Result<String> {
    HvigorJsonParser::new(source, locations)
      .bridge_json()
      .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))
  }
}
