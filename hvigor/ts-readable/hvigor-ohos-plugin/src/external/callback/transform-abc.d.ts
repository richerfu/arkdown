/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/external/callback/transform-abc.d.ts
 */
export interface TransformAbcConfig {
    isArkGuardEnabled: boolean;
}
export type TransformAbcCallback = (abcPath: string, config: TransformAbcConfig) => void | Promise<void>;
