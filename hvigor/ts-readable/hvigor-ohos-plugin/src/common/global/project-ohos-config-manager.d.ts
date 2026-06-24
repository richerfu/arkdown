/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/project-ohos-config-manager.d.ts
 */
import { AppOhosConfig, Overrides } from '../../options/build/app-ohos-config';
export declare class ProjectOhosConfigManager {
    private ohosConfig;
    loaderConfig(profile: AppOhosConfig): void;
    getOverrides(): Overrides | undefined;
    getConfig(): AppOhosConfig | undefined;
}
export declare const projectOhosConfigManager: ProjectOhosConfigManager;
