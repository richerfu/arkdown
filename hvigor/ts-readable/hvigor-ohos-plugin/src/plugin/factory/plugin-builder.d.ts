/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/plugin-builder.d.ts
 */
import { Module } from '@ohos/hvigor';
import { ProjectModelImpl } from '../../model/project/project-model-impl';
import { AbstractModulePlugin } from '../common/abstract-module-plugin';
import { HapPlugin } from '../hap-plugin';
import { HarPlugin } from '../har-plugin';
import { HspPlugin } from '../hsp-plugin';
import { CorePluginBuilder } from './core-plugin-builder';
export declare class PluginBuilder extends CorePluginBuilder {
    private _log;
    constructor(module: Module);
    createStageHapPlugin(): HapPlugin;
    createStageHspPlugin(): HspPlugin;
    createStageHarPlugin(): HarPlugin;
    initPluginParam(plugin: AbstractModulePlugin, isHapModule: boolean): AbstractModulePlugin;
    setUpModulePlugin(plugin: AbstractModulePlugin): void;
    createProjectModel(): ProjectModelImpl;
}
