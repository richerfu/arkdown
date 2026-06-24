/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/clean-ets-for-tgz.d.ts
 */
import { OhosLogger } from '../log/ohos-logger';
import { HvigorOptions } from '../options/hvigor-options';
/**
 * 指定场景下手动删除etsForTgz清理缓存
 * 不存在原缓存路径时删除etsForTgz
 * buildMode切换时删除etsForTgz
 *
 * @param options
 * @param logger
 */
export declare function cleanEtsForTgz(options: HvigorOptions, logger: OhosLogger): void;
