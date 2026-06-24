/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/plugin-factory.d.ts
 */
import { Module, Project } from '@ohos/hvigor';
import { AppPlugin } from '../app-plugin';
import { AbstractHapModulePlugin } from '../common/abstract-hap-module-plugin';
import { AbstractHarModulePlugin } from '../common/abstract-har-module-plugin';
import { HspPlugin } from '../hsp-plugin';
/**
 * 创建不同模型Plugin的简单工厂类
 *
 * @since 2022/2/23
 */
export declare class PluginFactory {
    /**
     * 获取AppPlugin实例
     *
     * @param project - 项目对象
     * @param isFaMode - 是否为FA模式，默认为false
     * @returns AppPlugin实例
     */
    static getAppPlugin(project: Project, isFaMode?: boolean): AppPlugin;
    /**
     * 获取AbstractHapModulePlugin实例
     *
     * @param module - 模块对象
     * @param isFaMode - 是否为FA模式，默认为false
     * @returns AbstractHapModulePlugin实例
     */
    static getHapPlugin(module: Module, isFaMode?: boolean): AbstractHapModulePlugin;
    /**
     * 获取AbstractHarModulePlugin实例
     *
     * @param module - 模块对象
     * @param isFaMode - 是否为FA模式，默认为false
     * @returns AbstractHarModulePlugin实例
     */
    static getHarPlugin(module: Module, isFaMode?: boolean): AbstractHarModulePlugin;
    /**
     * 获取HspPlugin实例
     *
     * @param module - 模块对象
     * @returns HspPlugin实例
     */
    static getHspPlugin(module: Module): HspPlugin;
}
