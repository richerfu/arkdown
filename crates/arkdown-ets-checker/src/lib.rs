//! Native ArkTS TypeScript service client. OXC owns parsing/transforms in
//! arkdown; the selected native compiler owns project type inference.

mod native;

pub use native::{
    DiagnosticPosition, NativeAnnotationConstant, NativeAnnotationInfo, NativeAnnotationProperty,
    NativeAnnotationTransformImport, NativeAnnotationTransformInfo, NativeAnnotationTransformNode,
    NativeArktsBuilderReceiverTypeFacts, NativeArktsPropertyTypeFacts,
    NativeArktsTransformTypeFacts, NativeCheckError, NativeCompilerOptions, NativeDiagnostic,
    NativeOhSdkCheckPlugin, NativeOhSdkClassCheckPlugin, NativeOhSdkConfig, NativeProgram,
    NativeProgramSourceGraph, NativeProgramSourceGraphFile, NativeResolvedTypeIdentity,
    NativeSymbol, NativeType, NativeTypeSession,
};
