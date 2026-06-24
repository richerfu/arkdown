/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/module-ohos-config-manager.d.ts
 */
import { BuildOption, ModuleOhosConfig } from '../../options/build/hap-ohos-config';
export declare class ModuleOhosConfigManager {
    private configMap;
    loaderConfig(moduleName: string, config: ModuleOhosConfig): void;
    getBuildOption(moduleName: string): BuildOption | undefined;
    getConfig(moduleName: string): ModuleOhosConfig | undefined;
}
export declare const moduleOhosConfigManager: ModuleOhosConfigManager;
