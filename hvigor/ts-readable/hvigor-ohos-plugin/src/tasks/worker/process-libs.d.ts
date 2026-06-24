/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/process-libs.d.ts
 */
import { NativeLibraryModel } from '../../model/cxx/native-library-model';
import { NapiLibFilterOpt, PkgSelection } from '../../options/build/build-opt';
import { Dependency } from '../../project/dependency/core/dependency-interface';
import { OhosLogger } from '../../utils/log/ohos-logger';
export interface ProcessLibsOptions {
    outputDir: string;
    filterRule?: NapiLibFilterOpt;
    allHarLibs: LibWithPkgInfo[];
    localLibs: LibWithPkgInfo[];
    outputLibs: LibWithPkgInfo[];
    profilePath: string;
    moduleName: string;
    cangjieLibs: LibWithPkgInfo[];
    collectAllLibs: boolean | undefined;
    readElf?: string;
    localOutputDir?: string;
    compileCommandsPath: string[] | undefined;
    isHarType: boolean | undefined;
    nativeOptionArg?: string | string[] | undefined;
    nativeLibs?: Map<string, NativeLibraryModel>[] | undefined;
}
export declare const buildLibs: (libsInfo: string[], pkg: Dependency, cwd: string) => LibWithPkgInfo[];
export declare const filterBySelection: (collections: LibWithPkgInfo[], select: PkgSelection[]) => LibWithPkgInfo[];
export declare const applyForSelectionRule: (collections: LibWithPkgInfo[], rule: PkgSelection) => void;
export declare function processLibs(config: ProcessLibsOptions, log: OhosLogger): Promise<void>;
export interface LibWithPkgInfo {
    name: string;
    path: string;
    relativePath: string;
    pkgName: string;
    pkgVersion: string;
    isLocal: boolean;
    selected: number;
}
