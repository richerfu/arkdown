/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/compile-node.d.ts
 */
/// <reference types="node" />
/// <reference types="node" />
import { DurationEvent, TaskDetails } from '@ohos/hvigor';
import { SpawnSyncOptions } from 'child_process';
import { NodeCommandBuilder } from '../builder/node-command-builder';
import { CodeType } from '../enum/code-type-enum';
import { LegacyAbilityModel } from '../model/ability/legacy-ability-model';
import { LoaderEnv } from '../options/loader-env';
import { OhosLogger } from '../utils/log/ohos-logger';
import { AbstractCompileNode } from './abstract/abstract-compile-node';
import { TargetTaskService } from './service/target-task-service';
/**
 * 调用loader编译新模型的js/ets源码
 *
 * @since 2022/1/10
 */
export declare class CompileNode extends AbstractCompileNode {
    private _log;
    private readonly _webpackPath;
    private readonly webpackConfig;
    constructor(taskService: TargetTaskService, codeType: CodeType, taskDetails: TaskDetails);
    initTaskDepends(): void;
    protected beforeTask(): Promise<void>;
    protected doTaskAction(): Promise<void>;
    protected doRealLoaderCompile(cmdEnv: LoaderEnv, callback: Function, callbackInput: unknown[], subDurationEvent: DurationEvent, targetWorkers?: number[]): Promise<void>;
    protected createNodeCommandBuilder(): NodeCommandBuilder;
    protected getInputData(cmdEnv: NodeJS.ProcessEnv, commandLine: string[]): {
        moduleName: string;
        taskName: string;
        commandLine: string[];
        commandOptions: SpawnSyncOptions;
    };
    private generateLoaderEnv;
    /**
     * 检查是否存在js模块依赖ets lib的情况
     *
     * @private
     */
    protected checkLocalDependency(abilityObjs: LegacyAbilityModel[], log: OhosLogger): void;
}
