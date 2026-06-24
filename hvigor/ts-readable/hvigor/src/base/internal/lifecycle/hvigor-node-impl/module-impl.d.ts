/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hvigor-node-impl/module-impl.d.ts
 */
import { Module, Project } from '../../../external/core/hvigor-core-node';
import { DefaultNodeImpl } from './default-node-impl';
/**
 * hvigor工程的基础module
 *
 * @since 2021/12/27
 */
export declare class ModuleImpl extends DefaultNodeImpl implements Module {
    classKind: string;
    private readonly _project;
    constructor(project: Project, moduleName: string, moduleDir: string);
    findModuleByName(moduleName: string): Module | undefined;
    getProject(): Project;
}
