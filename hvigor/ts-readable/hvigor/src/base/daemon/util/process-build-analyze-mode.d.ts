/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/process-build-analyze-mode.d.ts
 */
import { HvigorCliOptions } from '../../util/options/hvigor-cli-options';
/**
 * 处理历史变更已经deprecated的cli命令行&构建配置
 *
 * @param {HvigorCliOptions} hvigorCliOptions
 */
export declare function processLegacyConfig(hvigorCliOptions?: HvigorCliOptions): void;
