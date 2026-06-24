/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/module/module-model-impl.d.ts
 */
import { Module } from '@ohos/hvigor';
import { ModuleType } from '../../enum/module-type-enum';
import { ModuleJson } from '../../options/configure/module-json-options';
import { ProjectModel } from '../project/project-model';
import { TargetSourceSetModel } from '../source-set/source-set-model';
import { CoreModuleModelImpl } from './core-module-model-impl';
/**
 * hvigor 工程的module模块的数据管理对象
 *
 * @since 2022/2/23
 */
export declare class ModuleModelImpl extends CoreModuleModelImpl {
    constructor(module: Module, parentProject: ProjectModel);
    getDeviceTypes(): string[];
    initDefaultTargetSourceSet(): void;
    getModuleType(): ModuleType;
    getSourceSetByTargetName(targetName?: string): TargetSourceSetModel;
    getJsonObjByTargetName(targetName: string): ModuleJson.ModuleOptObj;
    getJsonPathByTargetName(targetName: string): string;
    isAtomicService(): boolean;
    isInstallFree(): boolean;
    /**
     * 根据模块的target和存储卡片信息的文件路径得到卡片信息
     * 1.在当前模块的资源目录中索引卡片配置文件
     * 2.如果使用了formExtensionModule配置项（卡片包），则会去对应的卡片包模块的资源目录索引卡片配置文件
     *
     * @param targetName 模块的target名
     * @param formPath 存储卡片信息的文件路径
     * @return ModuleJson.FormsObj 存储卡片们的信息的对象
     */
    getFormJsonObjByTargetName(targetName: string, formPath: string): ModuleJson.FormsObj;
    getPermission(): object;
    getCompressNativeLib(): unknown;
    isFormWidgetModule(targetName: string): boolean;
    isFormExtensionModule(targetName: string): boolean;
    hasFormConfig(targetName: string): boolean;
}
