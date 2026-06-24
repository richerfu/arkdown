/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/hvigor-native-build.d.ts
 */
export declare const HvigorNativeConfigFile = "hvigor_native_config.json";
export declare const readHvigorNativeConfigFile: (file: string) => Promise<HvigorNativeConfig | undefined>;
export interface HvigorNativeConfig {
    cleanCommands: string[][];
    buildCommands: string[][];
    libraries: NativeLibraries;
    buildFiles: string[];
    toolchain: NativeToolchainModel;
    extra?: string[];
}
export interface NativeLibraries {
    [key: string]: NativeLibrary;
}
export interface NativeToolchainModel {
    cCompilerExecutable: string;
    cppCompilerExecutable: string;
}
export interface NativeLibrary {
    toolchain?: string;
    abi?: string;
    artifactName?: string;
    output?: string;
    runtimeFiles?: string[];
}
