/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/import-check-plugin.d.ts
 */
import { ModuleInfo, PluginContext } from '@ohos/hvigor-arkts-base';
export declare function importCheckPlugin(): {
    name: string;
    resolveId: typeof resolveId;
    transform: typeof transform;
    moduleParsed: typeof moduleParsed;
};
/**
 * 解析路径
 * @param oriImportee  import语句的写法
 * @param importer  import引用者
 * @param resolvedId resolvedId
 * @param options   参数
 */
declare function resolveId(this: PluginContext, oriImportee: string, resolvedId: string, importer: string, options: any): void;
/**
 * transform文件
 * @param id 文件路径
 */
declare function transform(this: PluginContext, code: string, id: string): void;
declare function moduleParsed(this: PluginContext, moduleInfo: ModuleInfo): void;
declare const _default: {
    resolveId: typeof resolveId;
    transform: typeof transform;
    moduleParsed: typeof moduleParsed;
};
export default _default;
