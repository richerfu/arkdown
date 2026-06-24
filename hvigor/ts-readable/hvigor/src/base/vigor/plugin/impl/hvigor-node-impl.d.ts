/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/impl/hvigor-node-impl.d.ts
 */
import { NormalizedFile } from '../../../../common/util/hvigor-file-util';
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { Consumer } from '../../../external/interface/consumer';
import { HvigorTask } from '../../task/interface/hvigor-task';
import { Task } from '../../task/interface/task';
import { HvigorNode } from '../interface/hvigor-node';
import { Config } from '../interface/loader-profile';
/**
 * Hvigor Node节点实现类 (新)
 */
export declare class HvigorNodeImpl implements HvigorNode {
    private logger;
    private node;
    private allSubNodes;
    private parentNode;
    nodeDir: NormalizedFile;
    /**
     * 可能存在工程名与模块名相同的情况，此时 this.getNodeName() 会返回相同的名字
     * 因此需要多加一个字段区分，可以是 'project' 或 'module'
     */
    classKind: string;
    constructor(node: HvigorCoreNode);
    getNodeName(): string;
    getNodeNameInternal(): string;
    private _getNodeName;
    getNodePath(): string;
    getNodePathInternal(): string;
    private _getNodePath;
    getParentNode(): HvigorNode | undefined;
    getParentNodeInternal(): HvigorNode | undefined;
    private _getParentNode;
    getContext(pluginId: string): any;
    getContextInternal(pluginId: string): any;
    private _getContext;
    getAllPluginIds(): string[];
    getAllPluginIdsInternal(): string[];
    private _getAllPluginIds;
    getSubNodeByName(nodeName: string): HvigorNode | undefined;
    getSubNodeByNameInternal(nodeName: string): HvigorNode | undefined;
    private _getSubNodeByName;
    getTaskByName(taskName: string): Task | undefined;
    getTaskByNameInternal(taskName: string): Task | undefined;
    private _getTaskByName;
    registerTask(task: HvigorTask): HvigorTask;
    registerTaskInternal(task: HvigorTask): HvigorTask;
    private _registerTask;
    subNodes(callbackfn: (node: HvigorNode) => void): void;
    subNodesInternal(callbackfn: (node: HvigorNode) => void): void;
    private _subNodes;
    getNodeDir(): NormalizedFile;
    getNodeDirInternal(): NormalizedFile;
    private _getNodeDir;
    getConfigOpt(): Config;
    getConfigOptInternal(): Config;
    private _getConfigOpt;
    addExtraOption(key: string, value: any): void;
    addExtraOptionInternal(key: string, value: any): void;
    private _addExtraOption;
    getExtraOption(key: string): any;
    getExtraOptionInternal(key: string): any;
    private _getExtraOption;
    beforeNodeEvaluate(fn: Consumer<HvigorNode>): void;
    beforeNodeEvaluateInternal(fn: Consumer<HvigorNode>): void;
    private _beforeNodeEvaluate;
    afterNodeEvaluate(fn: Consumer<HvigorNode>): void;
    afterNodeEvaluateInternal(fn: Consumer<HvigorNode>): void;
    private _afterNodeEvaluate;
    getAllTasks(): Task[];
    getAllTasksInternal(): Task[];
    private _getAllTasks;
    private _loadParentNode;
    private _loadAllSubNodes;
}
