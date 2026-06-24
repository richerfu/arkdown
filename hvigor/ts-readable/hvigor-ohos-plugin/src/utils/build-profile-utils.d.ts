/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/build-profile-utils.d.ts
 */
import { TaskInputValue } from '@ohos/hvigor';
import { RuntimeOnlyObj } from '../options/build/build-opt';
import { ModuleTargetData } from '../tasks/data/hap-task-target-data';
import { OhosLogger } from './log/ohos-logger';
/**
 * 生成buildProfile.ets文件
 *
 * @param buildProfileData
 * @param targetData
 * @param _log
 */
export declare const normalizedFileData: (buildProfileData: object, targetData: ModuleTargetData, _log: OhosLogger) => string;
export declare const generateConstVariable: (buildProfileData: object, targetData: ModuleTargetData, _log: OhosLogger) => string;
export declare const generateExportClass: (buildProfileData: object) => string;
export declare const getRuntimeOnlyObjMap: (modulePath: string, runtimeOnlyObj: RuntimeOnlyObj | undefined, unionDependencyList: any[], suffix?: number | string) => Map<string, TaskInputValue>;
export declare const getCustomTypePathIfExists: (modulePath: string, type: string) => string | undefined;
