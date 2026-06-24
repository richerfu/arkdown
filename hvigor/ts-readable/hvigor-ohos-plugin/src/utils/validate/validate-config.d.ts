/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/validate-config.d.ts
 */
import { AppOhosConfig } from '../../options/build/app-ohos-config';
import { HapOhosConfig, HarOhosConfig } from '../../options/build/hap-ohos-config';
/**
 * 校验在hvigorfile.ts里配置的config.ohos字段的内容
 */
export declare function validateOhosProjectConfig(config: AppOhosConfig, filePath: string): void;
export declare function validateOhosModuleConfig(config: HapOhosConfig, filePath: string): void;
export declare function validateOhosHarModuleConfig(config: HarOhosConfig, filePath: string): void;
