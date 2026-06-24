/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/har-plugin-builder.d.ts
 */
import { AbstractPluginBuilder } from './abstract-plugin-builder';
import { Module } from '@ohos/hvigor';
import { TaskCreatorManager } from '../../tasks/task-creator';
import { AbstractHarModulePlugin } from '../common/abstract-har-module-plugin';
export declare class HarPluginBuilder extends AbstractPluginBuilder<AbstractHarModulePlugin> {
    private readonly module;
    private _creatorManager;
    constructor(module: Module, isFaMode?: boolean);
    get creatorManager(): TaskCreatorManager | undefined;
    set creatorManager(creatorManager: TaskCreatorManager | undefined);
    protected createPlugin(): AbstractHarModulePlugin;
    protected registerHooks(plugin: AbstractHarModulePlugin): void;
}
