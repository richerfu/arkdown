/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-compile-lite-node.d.ts
 */
/// <reference types="node" />
import { DurationEvent, FileSet } from '@ohos/hvigor';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { CompileNode } from '../compile-node';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 调用ace-loader编译老模型的js的瘦设备代码
 *
 * @since 2022/5/17
 */
export declare class LegacyCompileLiteNode extends CompileNode {
    private _logger;
    constructor(taskService: TargetTaskService);
    declareOutputFiles(): FileSet;
    taskShouldDo(): boolean;
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
    protected moveReleaseMap(targetData: ModuleTargetData, log: OhosLogger, abilityPath?: string): void;
    protected doRealLoaderCompile(cmdEnv: NodeJS.ProcessEnv, callback: Function, callbackInput: unknown[], subDurationEvent: DurationEvent, targetWorkers?: number[]): Promise<void>;
}
