/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/hsp-plugin-builder.d.ts
 */
import { AbstractPluginBuilder } from './abstract-plugin-builder';
import { Module } from '@ohos/hvigor';
import { TaskCreatorManager } from '../../tasks/task-creator';
import { HspPlugin } from '../hsp-plugin';
export declare class HspPluginBuilder extends AbstractPluginBuilder<HspPlugin> {
    private readonly module;
    private _creatorManager;
    constructor(module: Module, isFaMode?: boolean);
    get creatorManager(): TaskCreatorManager | undefined;
    set creatorManager(creatorManager: TaskCreatorManager | undefined);
    protected createPlugin(): HspPlugin;
    protected registerHooks(plugin: HspPlugin): void;
    /**
     * 初始化HSP插件的ohosConfig数据。
     *
     * @param module - 需要初始化的模块对象。
     * @returns 无返回值。
     */
    initHspPluginGlobalData(module: Module): void;
}
