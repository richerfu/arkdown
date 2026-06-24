/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/tree-node.d.ts
 */
export declare class TreeNode<T> {
    key: string;
    value: T;
    children: TreeNode<T>[];
    constructor(key: string, value: T);
}
export declare class Tree<T> {
    root: TreeNode<T>;
    constructor(key: string, value: T);
    findNodeByKeyDFS(key: string, node: TreeNode<T>): TreeNode<T> | undefined;
    findNodeByKeyBFS(key: string): TreeNode<T> | undefined;
    findParentNodeByKey(key: string): TreeNode<T> | undefined;
    addNode(key: string, value: T, parentKey: string): boolean;
}
