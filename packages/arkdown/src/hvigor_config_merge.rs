use napi::bindgen_prelude::*;
use napi::JsValue;

/// Lossless V8 bridge for hvigor configuration merging. Rust owns the merge
/// algorithm, while JavaScript functions and other live values retain their
/// original identities in the returned object graph.
pub(crate) struct HvigorConfigMergeNapiAdapter {
  env: Env,
}

impl HvigorConfigMergeNapiAdapter {
  pub(crate) fn new(env: Env) -> Self {
    Self { env }
  }

  pub(crate) fn merge<'env>(
    &self,
    base: Unknown<'env>,
    overlay: Unknown<'env>,
  ) -> napi::Result<Unknown<'env>> {
    if overlay.get_type()? == ValueType::Undefined {
      return Ok(base);
    }
    if !Self::is_mergeable_object(&base)? || !Self::is_mergeable_object(&overlay)? {
      return Ok(overlay);
    }

    let base_object = unsafe { base.cast::<Object<'env>>()? };
    let overlay_object = unsafe { overlay.cast::<Object<'env>>()? };
    let mut merged = Object::new(&self.env)?;
    self.copy_own_entries(&base_object, &mut merged)?;
    for key in Object::keys(&overlay_object)? {
      if !overlay_object.has_own_property(&key)? {
        continue;
      }
      let Some(overlay_value) = overlay_object.get::<Unknown<'env>>(&key)? else {
        continue;
      };
      let value = match merged.get::<Unknown<'env>>(&key)? {
        Some(base_value) => self.merge(base_value, overlay_value)?,
        None => overlay_value,
      };
      merged.set(&key, value)?;
    }
    let raw = unsafe { Object::to_napi_value(self.env.raw(), merged)? };
    // SAFETY: `raw` was created from an object in this adapter's Env and is
    // returned synchronously before that Env can be released.
    Ok(unsafe { Unknown::from_raw_unchecked(self.env.raw(), raw) })
  }

  fn is_mergeable_object(value: &Unknown<'_>) -> napi::Result<bool> {
    Ok(value.get_type()? == ValueType::Object && !value.is_array()?)
  }

  fn copy_own_entries<'env>(
    &self,
    source: &Object<'env>,
    destination: &mut Object<'env>,
  ) -> napi::Result<()> {
    for key in Object::keys(source)? {
      if !source.has_own_property(&key)? {
        continue;
      }
      if let Some(value) = source.get::<Unknown<'env>>(&key)? {
        destination.set(&key, value)?;
      }
    }
    Ok(())
  }
}
