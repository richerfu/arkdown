/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/native-codemodel.d.ts
 */
import { NativeTarget } from './native-codemodel-target';
export interface NativeCodemodel {
    configurations?: Configuration[];
    kind?: string;
    paths?: Paths;
    version?: Version;
}
export interface Configuration {
    directories?: Directory[];
    name?: string;
    projects?: Project[];
    targets: NativeTarget[];
}
export interface Directory {
    build?: string;
    minimumCMakeVersion?: MinimumCMakeVersion;
    projectIndex?: number;
    source?: string;
    targetIndexes?: number[];
}
export interface MinimumCMakeVersion {
    string?: string;
}
export interface Project {
    directoryIndexes?: number[];
    name?: string;
    targetIndexes?: number[];
}
export interface Paths {
    build?: string;
    source?: string;
}
export interface Version {
    major?: number;
    minor?: number;
}
