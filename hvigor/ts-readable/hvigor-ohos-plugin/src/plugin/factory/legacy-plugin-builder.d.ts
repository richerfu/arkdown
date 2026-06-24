/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/legacy-plugin-builder.d.ts
 */
import { Module } from '@ohos/hvigor';
import { AbstractModulePlugin } from '../common/abstract-module-plugin';
import { LegacyHapPlugin } from '../legacy/legacy-hap-plugin';
import { LegacyHarPlugin } from '../legacy/legacy-har-plugin';
import { CorePluginBuilder } from './core-plugin-builder';
export declare class LegacyPluginBuilder extends CorePluginBuilder {
    private _log;
    constructor(module: Module);
    setUpModulePlugin(plugin: AbstractModulePlugin): void;
    createFaHapPlugin(): LegacyHapPlugin;
    createFaHarPlugin(): LegacyHarPlugin;
    initPluginParam(plugin: AbstractModulePlugin, isHapModule: boolean): AbstractModulePlugin;
    private createProjectModel;
}
