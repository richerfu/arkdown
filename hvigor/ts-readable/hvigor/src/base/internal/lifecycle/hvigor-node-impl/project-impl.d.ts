/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hvigor-node-impl/project-impl.d.ts
 */
import { Module, Project } from '../../../external/core/hvigor-core-node';
import { Config } from '../../../vigor/plugin/interface/loader-profile';
import { DefaultNodeImpl } from './default-node-impl';
/**
 * hvigor工程的基础root module
 *
 * @since 2021/12/27
 */
export declare class ProjectImpl extends DefaultNodeImpl implements Project {
    classKind: string;
    private readonly _subProjects;
    private _config;
    constructor(projectName: string, moduleDir: string);
    findModuleByName(moduleName: string): Module | undefined;
    getProject(): Project;
    getSubModules(): Map<string, Module>;
    addSubModule(module: Module): void;
    getAllSubModules(): Module[];
    loadConfig(config: any): void;
    getConfigOpt(): Config;
}
