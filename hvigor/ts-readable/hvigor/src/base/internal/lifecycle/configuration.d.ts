/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/configuration.d.ts
 */
import { HvigorCoreNode } from '../../external/core/hvigor-core-node';
import { DurationEvent } from '../../metrics/event/duration-event';
/**
 * Config整个hvigor的项目，进行数据绑定
 *
 */
export declare function configuration(parentEvent: DurationEvent): Promise<HvigorCoreNode>;
