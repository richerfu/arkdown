/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/module/legacy-module-model-impl.d.ts
 */
import { Module } from '@ohos/hvigor';
import { ModuleType } from '../../enum/module-type-enum';
import { ConfigJson } from '../../options/configure/config-json-options';
import { LegacyAbilityModel } from '../ability/legacy-ability-model';
import { ProjectModel } from '../project/project-model';
import { LegacyTargetSourceSetModel } from '../source-set/source-set-model';
import { CoreModuleModelImpl } from './core-module-model-impl';
import ConfigOptObj = ConfigJson.ConfigOptObj;
export declare class LegacyModuleModelImpl extends CoreModuleModelImpl {
    private _legacyAbilitiesMap;
    constructor(module: Module, parentProject: ProjectModel);
    private static validateSameNameAbility;
    initDefaultTargetSourceSet(): void;
    /**
     * 获取需要进行编译的ability信息
     */
    getLegacyAbilities(targetName?: string): LegacyAbilityModel[];
    getModuleType(targetName?: string): ModuleType;
    getDeviceTypes(): string[];
    getSourceSetByTargetName(targetName?: string): LegacyTargetSourceSetModel;
    getJsonObjByTargetName(targetName: string): ConfigOptObj;
    getJsonPathByTargetName(targetName: string): string;
    isAtomicService(): boolean;
    isInstallFree(): boolean;
    private initAbilityInfo;
    /**
     * 校验config.json中的forms中name字段为唯一
     *
     * @param legacyAbilitiesObjs
     * @private
     */
    private static validateFormUnique;
    getPermission(): object;
    /**
     * FA模式不涉及分布式卡片包模块
     *
     * @return {boolean} 固定返回false
     */
    isFormWidgetModule(targetName: string): boolean;
    /**
     * FA模式不涉及分布式卡片包模块
     *
     * @return {boolean} 固定返回false
     */
    isFormExtensionModule(targetName: string): boolean;
    /**
     * FA模式不涉及
     *
     * @return {boolean} 固定返回false
     */
    hasFormConfig(targetName: string): boolean;
}
