/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/const/hook-const.d.ts
 */
import { Hvigor } from '../../base/external/core/hvigor';
import { HvigorConfig } from '../../base/external/core/hvigor-config';
import { BuildResult } from '../../base/external/models/build-result';
import { HvigorNode } from '../../base/vigor/plugin/interface/hvigor-node';
export declare enum HookType {
    configEvaluated = "configEvaluated",
    nodesInitialized = "nodesInitialized",
    beforeNodeEvaluate = "beforeNodeEvaluate",
    afterNodeEvaluate = "afterNodeEvaluate",
    nodesEvaluated = "nodesEvaluated",
    taskGraphResolved = "taskGraphResolved",
    buildFinished = "buildFinished",
    onWatchWorkerMessage = "onWatchWorkerMessage"
}
export interface HookArgMap {
    [HookType.configEvaluated]: HvigorConfig;
    [HookType.nodesInitialized]: Hvigor;
    [HookType.beforeNodeEvaluate]: HvigorNode;
    [HookType.afterNodeEvaluate]: HvigorNode;
    [HookType.nodesEvaluated]: Hvigor;
    [HookType.taskGraphResolved]: Hvigor;
    [HookType.buildFinished]: BuildResult;
    [HookType.onWatchWorkerMessage]: any;
}
