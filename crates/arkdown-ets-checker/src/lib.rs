//! Native ArkTS TypeScript service client. OXC owns parsing/transforms in
//! arkdown; the selected native compiler owns project type inference.

mod native;

pub use native::{
    DiagnosticPosition, NativeAnnotationConstant, NativeAnnotationInfo, NativeAnnotationProperty,
    NativeAnnotationTransformImport, NativeAnnotationTransformInfo, NativeAnnotationTransformNode,
    NativeArktsBuilderReceiverTypeFacts, NativeArktsPropertyTypeFacts,
    NativeArktsTransformTypeFacts, NativeArktsTransformTypeQuery, NativeCheckError,
    NativeCompilerOptions, NativeDiagnostic, NativeOhSdkCheckPlugin, NativeOhSdkClassCheckPlugin,
    NativeOhSdkConfig, NativeOhSdkPluginHost, NativeOhSdkPluginRequest, NativeOhSdkPluginResponse,
    NativeProgram, NativeProgramSourceGraph, NativeProgramSourceGraphFile,
    NativeResolvedTypeIdentity, NativeSymbol, NativeType, NativeTypeQueryRange, NativeTypeSession,
};
