/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/plugin-inspector.d.ts
 */
import { AbstractHapModulePlugin } from '../common/abstract-hap-module-plugin';
import { AbstractHarModulePlugin } from '../common/abstract-har-module-plugin';
export declare class PluginInspector {
    private static _log;
    /**
     * 校验feature模块的entryModules中的字段是否合法的entry
     * @param hapPlugin
     * @public
     */
    static checkFeatureEntryModules(hapPlugin: AbstractHapModulePlugin): void;
    /**
     * 检查原子化服务是否依赖hsp
     * @param hapPlugin
     * @public
     */
    static checkAtomicServiceHspModules(hapPlugin: AbstractHarModulePlugin): void;
}
