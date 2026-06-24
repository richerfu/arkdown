/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/hvigor-ohos-plugin-adaptor.d.ts
 */
import { HvigorErrorAdaptor } from './hvigor-error-adaptor';
/**
 * hvigor-ohos-plugin插件的适配器
 */
export declare class HvigorOhosPluginAdaptor extends HvigorErrorAdaptor {
    getErrorCodeJsonPath(): string[];
}
