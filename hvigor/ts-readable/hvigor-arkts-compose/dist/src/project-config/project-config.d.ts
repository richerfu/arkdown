/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/project-config.d.ts
 */
import { ProjectMeta } from './project-meta';
import { LogInfo } from './log-info';
import { BuildEnvironment } from './build-environment';
import { BuildOption } from './build-option/build-option';
import { ResourceInfo } from './resource-info';
import { TestInfo } from './test-info';
import { PerfInfo } from './perf-info';
import { MockInfo } from './mock-info';
export type ModuleInfoType = {
    moduleName: string;
    modulePkgPath: string;
    belongProjectPath: string;
    isBuildCacheDir?: boolean;
};
export type ArkCompileProjectModelType = Record<string, ModuleInfoType>;
export interface ProjectConfig extends ProjectMeta, LogInfo, BuildEnvironment, BuildOption, ResourceInfo, TestInfo, PerfInfo, MockInfo {
}
