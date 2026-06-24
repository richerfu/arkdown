/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/har-plugin.d.ts
 */
import { Module } from '@ohos/hvigor';
import { AbstractHarModulePlugin } from './common/abstract-har-module-plugin';
/**
 * 对外暴露har级别的接口和任务的plugin
 *
 * @since 2021/12/16
 */
export declare class HarPlugin extends AbstractHarModulePlugin {
    constructor(module: Module);
    initHarTasks(): void;
}
