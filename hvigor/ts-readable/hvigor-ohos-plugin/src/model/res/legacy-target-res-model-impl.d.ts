/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/legacy-target-res-model-impl.d.ts
 */
import { ConfigJson } from '../../options/configure/config-json-options';
import { CoreResModelImpl } from './core-res-model-impl';
import ConfigOptObj = ConfigJson.ConfigOptObj;
import { LegacyModuleTargetRes } from './res-model';
/**
 * Fa Module 的不同target对应的资源集合
 *
 * @since 2022/2/23
 */
export declare class LegacyModuleTargetResImpl extends CoreResModelImpl implements LegacyModuleTargetRes {
    protected _configJsonOpt: ConfigOptObj | undefined;
    constructor(sourceSetRoot: string);
    getConfigJsonOpt(): ConfigOptObj;
    hasLiteDevice(): boolean | undefined;
    supportLiteDeviceModule(): boolean;
}
