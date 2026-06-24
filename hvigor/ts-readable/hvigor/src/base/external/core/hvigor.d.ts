/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/core/hvigor.d.ts
 */
import { Parameter } from '../../data/parameters';
import { HvigorNode } from '../../vigor/plugin/interface/hvigor-node';
import { Consumer } from '../interface/consumer';
import { BuildResult } from '../models/build-result';
import { HvigorConfig } from './hvigor-config';
import { Project } from './hvigor-core-node';
/**
 * hvigor向外部暴露的hvigor对象 通过此对象可以操作hvigor运行中的内容
 *
 * @since 2024-03-13
 */
export declare class Hvigor {
    private hvigorLifecycleHook;
    getRootNode(): HvigorNode;
    private _getRootNode;
    getAllNodes(): HvigorNode[];
    private _getAllNodes;
    getNodeByName(nodeName: string, classKind?: string): HvigorNode | undefined;
    private _getNodeByName;
    getHvigorConfig(): HvigorConfig;
    private _getHvigorConfig;
    /**
     * 获取hvgior命令配置参数
     * @return {Parameter} Parameter对象
     */
    getParameter(): Parameter;
    private _getParameter;
    /**
     * 所有的node添加一个beforeNode hook
     * @param fn 方法的入参HvigorNode
     */
    beforeNodeEvaluate(fn: Consumer<HvigorNode>): void;
    private _beforeNodeEvaluate;
    /**
     * 为所有的node添加一个afterNode hook
     * @param fn 方法的入参HvigorNode
     */
    afterNodeEvaluate(fn: Consumer<HvigorNode>): void;
    private _afterNodeEvaluate;
    configEvaluated(fn: Consumer<HvigorConfig>): void;
    private _configEvaluated;
    nodesInitialized(fn: Consumer<Hvigor>): void;
    private _nodesInitialized;
    nodesEvaluated(fn: Consumer<Hvigor>): void;
    private _nodesEvaluated;
    taskGraphResolved(fn: Consumer<Hvigor>): void;
    private _taskGraphResolved;
    buildFinished(fn: Consumer<BuildResult>): void;
    private _buildFinished;
    onWatchWorkerMessage(fn: Consumer<any>): void;
    private _onWatchWorkerMessage;
    getCommandEntryTask(): string[] | undefined;
    private _getCommandEntryTask;
    /**
     * 判断是否是命令入口Task
     *
     * @param {string} taskName 任务名
     * @return {string[]} commandEntryTasks Task集合
     *
     */
    isCommandEntryTask(taskName: string): boolean;
    private _isCommandEntryTask;
    /**
     * 返回在命令行中传递的额外参数
     *
     * @returns {Map<string, string>}
     * @deprecated 已过时，为兼容历史版本而保留不再推荐使用 推荐使用hvigor.getParameter()方法代替
     */
    getExtraConfig(): Map<string, string>;
    private _getExtraConfig;
    /**
     * 返回Project模型
     *
     * @returns {Project | undefined}
     * @deprecated 已过时，为兼容历史版本而保留不再推荐使用
     */
    getProject(): Project | undefined;
    private _getProject;
    getHvigorUserHome(): string;
    private _getHvigorUserHome;
    getHvigorVersion(): string;
    private _getHvigorVersion;
}
export declare const hvigor: Hvigor;
