/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/router-map-plugin.d.ts
 */
import { PluginContext } from '@ohos/hvigor-arkts-base';
export declare function routerMapPlugin(): {
    name: string;
    shouldInvalidCache: typeof shouldInvalidCache;
};
declare function shouldInvalidCache(this: PluginContext, options: any): boolean;
export {};
