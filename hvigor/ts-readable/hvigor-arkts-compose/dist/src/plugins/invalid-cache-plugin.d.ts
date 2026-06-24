/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/invalid-cache-plugin.d.ts
 */
import { PluginContext } from '@ohos/hvigor-arkts-base';
/**
 * 用于判断缓存是否失效的插件
 */
export declare function invalidCachePlugin(): {
    name: string;
    buildStart: typeof buildStart;
    shouldTransformCachedModule: typeof shouldTransformCachedModule;
    buildEnd: typeof buildEnd;
    beforeBuild: typeof beforeBuild;
    beforeBuildEnd: typeof beforeBuildEnd;
    cleanUp(): void;
};
declare function buildEnd(this: PluginContext): void;
declare function beforeBuildEnd(this: PluginContext): void;
declare function buildStart(this: PluginContext, { plugins }: any): void;
declare function beforeBuild(this: PluginContext): void;
/**
 * 是否需要重新transform被缓存的模块
 * @param this
 * @param options
 * @returns
 */
declare function shouldTransformCachedModule(this: PluginContext, options: any): Promise<boolean>;
export {};
