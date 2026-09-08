//! Typed JavaScript callback bridge used by the Rolldown plugin adapter.
//!
//! This follows `rolldown_binding::types::js_callback`: Rust owns hook
//! scheduling while JavaScript callbacks may return either a value or a
//! Promise. Invalid return types become ordinary N-API errors instead of fatal
//! exceptions.

use std::{future::Future, ptr, sync::Arc};

use napi::{
  bindgen_prelude::{
    Either, FromNapiValue, JsValuesTupleIntoVec, Promise, TypeName, ValidateNapiValue,
  },
  sys,
  threadsafe_function::ThreadsafeFunction,
  Status, ValueType,
};

pub struct InvalidReturnValue {
  value_type: ValueType,
}

impl TypeName for InvalidReturnValue {
  fn type_name() -> &'static str {
    "InvalidReturnValue"
  }

  fn value_type() -> ValueType {
    ValueType::Unknown
  }
}

impl ValidateNapiValue for InvalidReturnValue {
  unsafe fn validate(
    _env: sys::napi_env,
    _value: sys::napi_value,
  ) -> napi::Result<sys::napi_value> {
    Ok(ptr::null_mut())
  }
}

impl FromNapiValue for InvalidReturnValue {
  unsafe fn from_napi_value(env: sys::napi_env, value: sys::napi_value) -> napi::Result<Self> {
    Ok(Self {
      value_type: napi::type_of!(env, value)?,
    })
  }
}

pub type MaybeAsyncJsReturn<Ret> = Either<Either<Promise<Ret>, Ret>, InvalidReturnValue>;

pub type MaybeAsyncJsCallback<T, Args, Ret> =
  Arc<ThreadsafeFunction<T, MaybeAsyncJsReturn<Ret>, Args, Status, false, true>>;

pub trait MaybeAsyncJsCallbackExt<T, Ret> {
  fn await_call(&self, value: T) -> impl Future<Output = napi::Result<Ret>> + Send;
}

impl<T, Args, Ret, const WEAK: bool> MaybeAsyncJsCallbackExt<T, Ret>
  for Arc<ThreadsafeFunction<T, MaybeAsyncJsReturn<Ret>, Args, Status, false, WEAK>>
where
  T: 'static + Send,
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue + TypeName,
  MaybeAsyncJsReturn<Ret>: FromNapiValue,
{
  async fn await_call(&self, value: T) -> napi::Result<Ret> {
    match self.call_async_catch(value).await? {
      Either::A(Either::A(promise)) => promise.await,
      Either::A(Either::B(value)) => Ok(value),
      Either::B(invalid) => Err(napi::Error::new(
        Status::InvalidArg,
        format!(
          "JavaScript callback returned {}, expected {}",
          js_type_name(invalid.value_type),
          js_type_name(Ret::value_type())
        ),
      )),
    }
  }
}

fn js_type_name(value_type: ValueType) -> &'static str {
  match value_type {
    ValueType::Undefined => "undefined",
    ValueType::Null => "null",
    ValueType::Boolean => "boolean",
    ValueType::Number => "number",
    ValueType::String => "string",
    ValueType::Symbol => "symbol",
    ValueType::Object => "object",
    ValueType::Function => "function",
    ValueType::External => "external",
    ValueType::BigInt => "bigint",
    ValueType::Unknown => "unknown",
  }
}
