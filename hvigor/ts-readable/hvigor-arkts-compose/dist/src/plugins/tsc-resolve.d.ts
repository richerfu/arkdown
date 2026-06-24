/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/tsc-resolve.d.ts
 */
import { PluginContext } from '@ohos/hvigor-arkts-base';
/**
 * 复用 sdk tsc 的路径解析信息
 */
export declare function tscResolve(options?: {
    caseSensitiveCheck?: boolean;
}): {
    name: string;
    buildStart: (this: PluginContext) => void;
    load: (this: PluginContext, id: string) => string | null;
    resolveId: typeof resolveId;
    beforeBuildEnd: () => void;
    cleanUp: () => void;
};
declare function resolveId(this: any, source: string, importer: string | undefined, options: any): Promise<{
    id: string;
    external: boolean;
    meta: object;
} | FinalResolved | null>;
interface FinalResolved {
    id: string;
    external?: boolean;
    isVirtualFile?: boolean;
}
export {};
