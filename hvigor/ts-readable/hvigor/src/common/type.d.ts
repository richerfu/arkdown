/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/type.d.ts
 */
export type Dependency = Record<string, string>;
export type PackageJson = {
    dependencies: Dependency;
    version?: string;
};
export type HvigorConfigJson = {
    modelVersion: string;
    dependencies: Dependency;
    properties?: PropertiesObj;
};
export type PropertiesObj = {
    'hvigor.dependency.useNpm'?: boolean;
};
