/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/native-cmake-files.d.ts
 */
export interface NativeCMakeFiles {
    kind: string;
    version: {
        major: number;
        minor: number;
    };
    paths: CMakeFilesPaths;
    inputs: InputFile[];
}
export interface CMakeFilesPaths {
    build: string;
    source: string;
}
export interface InputFile {
    path: string;
    isGenerated?: boolean;
    isExternal?: boolean;
    isCMake?: boolean;
}
