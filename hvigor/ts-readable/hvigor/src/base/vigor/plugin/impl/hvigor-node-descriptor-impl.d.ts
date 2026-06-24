/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/impl/hvigor-node-descriptor-impl.d.ts
 */
import { HvigorNodeDescriptor } from '../interface/hvigor-node-descriptor';
export declare class HvigorNodeDescriptorImpl implements HvigorNodeDescriptor {
    name: string;
    srcPath: string;
    extraOptions: Map<string, any>;
    private childrenNode;
    private rootNode;
    constructor(name: string, srcPath: string, childrenNode: HvigorNodeDescriptor[] | undefined, rootNode: HvigorNodeDescriptor | undefined);
    getChildNode(): HvigorNodeDescriptor[] | undefined;
    getChildNodeInternal(): HvigorNodeDescriptor[] | undefined;
    private _getChildNode;
    getRootNode(): HvigorNodeDescriptor;
    getRootNodeInternal(): HvigorNodeDescriptor;
    private _getRootNode;
    setChildNode(childrenNode: HvigorNodeDescriptor[] | undefined): void;
}
