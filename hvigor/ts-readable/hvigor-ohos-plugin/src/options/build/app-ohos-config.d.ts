/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/build/app-ohos-config.d.ts
 */
import { ProjectBuildProfile } from './project-build-profile';
import SigningConfigBuildOpt = ProjectBuildProfile.SigningConfigBuildOpt;
export type AppOhosConfig = {
    overrides?: Overrides;
};
export type AppOpt = {
    vendor: string;
    versionCode?: number;
    versionName?: string;
    bundleType?: string;
    bundleName?: string;
    icon?: string;
    label?: string;
};
export type Overrides = {
    appOpt?: AppOpt;
    signingConfig?: SigningConfigBuildOpt;
};
