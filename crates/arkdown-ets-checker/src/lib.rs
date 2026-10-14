//! Native ArkTS TypeScript service client. OXC owns parsing/transforms in
//! arkdown; the selected native compiler owns project type inference.

mod native;

pub use native::{
    DiagnosticPosition, NativeAnnotationConstant, NativeAnnotationInfo, NativeAnnotationProperty,
    NativeAnnotationTransformImport, NativeAnnotationTransformInfo, NativeAnnotationTransformNode,
    NativeArktsBuildDiagnostics, NativeArktsBuilderReceiverTypeFacts, NativeArktsNormalDiagnostics,
    NativeArktsPropertyTypeFacts, NativeArktsTransformTypeFacts, NativeArktsTransformTypeQuery,
    NativeCheckError, NativeCompilerOptions, NativeDiagnostic, NativeOhSdkCheckPlugin,
    NativeOhSdkClassCheckPlugin, NativeOhSdkConfig, NativeOhSdkPluginCompletion,
    NativeOhSdkPluginHost, NativeOhSdkPluginRequest, NativeOhSdkPluginResponse,
    NativeOhSdkPluginStats, NativeProcessedSource, NativeProgram, NativeProgramFileChanges,
    NativeProgramSourceGraph, NativeProgramSourceGraphFile, NativeResolvedTypeIdentity,
    NativeSourcePreprocessor, NativeSymbol, NativeType, NativeTypeQueryRange, NativeTypeSession,
};
