/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/options/project-structure-opt.d.ts
 */
export interface ModuleOpt {
    name: string;
    srcPath: string;
}
export interface ProjectStructureOpt {
    modules: ModuleOpt[];
}
