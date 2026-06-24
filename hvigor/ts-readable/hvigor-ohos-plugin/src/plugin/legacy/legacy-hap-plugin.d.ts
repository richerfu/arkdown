/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/legacy/legacy-hap-plugin.d.ts
 */
import { Module } from '@ohos/hvigor';
import { AbstractHapModulePlugin } from '../common/abstract-hap-module-plugin';
/**
 * 对外暴露hap级别的接口和任务的plugin
 *
 * @since 2021/12/16
 */
export declare class LegacyHapPlugin extends AbstractHapModulePlugin {
    constructor(module: Module);
    initHapTasks(): void;
}
