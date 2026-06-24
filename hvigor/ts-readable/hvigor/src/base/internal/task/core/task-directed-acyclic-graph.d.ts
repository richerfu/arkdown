/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-directed-acyclic-graph.d.ts
 */
export declare class TaskDirectedAcyclicGraph {
    private _out;
    private _in;
    /**
     * 添加一个图中的节点
     *
     * @param {string} nodeKey
     */
    addNode(nodeKey: string): void;
    /**
     * 添加两个节点之间的边,为了保证为一个有向无环图，故添加时需要判断两点之间是否已包含边
     *
     * @param {string} originKey
     * @param {string} targetKey
     * @returns {boolean}
     */
    addEdge(originKey: string, targetKey: string): boolean;
    private _hasEdge;
    /**
     * 删除图中节点,同时要删除关联的起始和结束节点
     *
     * @param {string} nodeKey
     */
    removeNode(nodeKey: string): void;
    /**
     * 清空DAG图
     */
    clear(): void;
    /**
     * 根据节点的key获取其子节点
     *
     * @param {string} nodeKey
     * @returns {Set<string>}
     */
    getChildren(nodeKey: string): Set<string>;
    /**
     * 根据节点的key获取其父节点
     *
     * @param {string} nodeKey
     * @return {Set<string>}
     */
    getParent(nodeKey: string): Set<string>;
    /**
     * 获取所有入度为0的起始节点
     *
     * @returns {Set<string>}
     */
    getAllStartNodes(): Set<string>;
    /**
     * 获取所有出度为0的叶子节点
     *
     * @returns {Set<string>}
     */
    getAllEndNodes(): Set<string>;
    /**
     * 检查是否有环
     */
    checkCircle(): {
        hasCircle: boolean;
        circlePath: string[];
    };
    private dfsCircleCheck;
    private findAllZeroEdgeNodes;
}
/**
 * 整个工程需要一个全局单一的DAG图用来保存和执行模块之间的任务依赖
 */
export declare const projectTaskDag: TaskDirectedAcyclicGraph;
