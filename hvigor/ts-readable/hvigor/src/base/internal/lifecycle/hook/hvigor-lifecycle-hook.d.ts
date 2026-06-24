/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hook/hvigor-lifecycle-hook.d.ts
 */
import { HookArgMap, HookType } from '../../../../common/const/hook-const';
import { Consumer } from '../../../external/interface/consumer';
import { DurationEvent } from '../../../metrics/event/duration-event';
/**
 * hvigor生命周期中的hook点存放对象
 */
export declare class HvigorLifecycleHook {
    private static instance;
    private configEvaluatedFnList;
    private nodesInitializedFnList;
    private beforeNodeEvaluateFnList;
    private afterNodeEvaluateFnList;
    private nodesEvaluatedFnList;
    private taskGraphResolvedFnList;
    private buildFinishedFnList;
    private onWatchWorkerMessageFnList;
    private strategies;
    /**
     * 实现单例模式
     */
    static getInstance(): HvigorLifecycleHook;
    runFn<T>(array: Consumer<T>[], arg: T): Promise<void>;
    /**
     *
     * 运行一个hook上所有的方法
     *
     * @param hookType hook点类型
     * @param arg 方法的入参
     * @param parentEvent
     */
    runHook<K extends HookType>(hookType: K, arg: HookArgMap[K], parentEvent: DurationEvent): Promise<void>;
    configEvaluated(fn: Consumer<HookArgMap[HookType.configEvaluated]>): void;
    nodesInitialized(fn: Consumer<HookArgMap[HookType.nodesInitialized]>): void;
    beforeNodeEvaluate(fn: Consumer<HookArgMap[HookType.beforeNodeEvaluate]>): void;
    afterNodeEvaluate(fn: Consumer<HookArgMap[HookType.afterNodeEvaluate]>): void;
    nodesEvaluated(fn: Consumer<HookArgMap[HookType.nodesEvaluated]>): void;
    taskGraphResolved(fn: Consumer<HookArgMap[HookType.taskGraphResolved]>): void;
    buildFinished(fn: Consumer<HookArgMap[HookType.buildFinished]>): void;
    onWatchWorkerMessage(fn: Consumer<HookArgMap[HookType.onWatchWorkerMessage]>): void;
    clear(): void;
}
