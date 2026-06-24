/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/core/default-module-dependency.d.ts
 */
import { DefaultNpmDependency } from './default-npm-dependency';
import { DependencyBuilder } from './dependency-builder';
/**
 * module依赖的工程内的本地har模块
 *
 * @since 2022/5/7
 */
export declare class DefaultModuleDependency extends DefaultNpmDependency {
    private readonly _module;
    constructor(dependencyBuilder: DependencyBuilder);
    getModuleName(): string;
    getModule(): import("@ohos/hvigor").HvigorCoreNode;
}
