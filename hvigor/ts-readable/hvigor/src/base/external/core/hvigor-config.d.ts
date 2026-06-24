/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/core/hvigor-config.d.ts
 */
import { ProjectImpl } from '../../internal/lifecycle/hvigor-node-impl/project-impl';
import { HvigorNodeDescriptor } from '../../vigor/plugin/interface/hvigor-node-descriptor';
/**
 * hvigor配置对象 用于提供给用户实现动态加载模块等功能
 */
export declare class HvigorConfig {
    private rootNodeDescriptor;
    private allChildrenNodeDescriptor;
    includeNode(name: string, srcPath: string, extraOptions?: Record<string, any>): void;
    private _includeNode;
    excludeNodeByName(name: string): void;
    private _excludeNodeByName;
    getAllNodeDescriptor(): HvigorNodeDescriptor[];
    private _getAllNodeDescriptor;
    getRootNodeDescriptor(): HvigorNodeDescriptor;
    private _getRootNodeDescriptor;
    getNodeDescriptorByName(name: string): HvigorNodeDescriptor | undefined;
    private _getNodeDescriptorByName;
}
export declare let hvigorConfig: HvigorConfig;
export declare function hvigorConfigInit(project: ProjectImpl): void;
