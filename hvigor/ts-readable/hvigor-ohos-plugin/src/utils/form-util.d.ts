/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/form-util.d.ts
 */
import { ModuleModel } from '../model/module/module-model';
import { ModuleTargetRes } from '../model/res/res-model';
import { ModuleJson } from '../options/configure/module-json-options';
import { TargetTaskService } from '../tasks/service/target-task-service';
import ModuleOptObj = ModuleJson.ModuleOptObj;
export declare class FormUtil {
    /**
     * 在模块的目录下搜索卡片的路径，返回第一个找到的，没找的话就undefined
     *
     * @param {ModuleModel} moduleModel
     * @param targetName
     * @returns {string | undefined} form_config.json路径
     */
    static getFormConfigPathInModuleRes(moduleModel: ModuleModel, targetName: string): string | undefined;
    /**
     * 返回完整的卡片路径
     *
     * @param moduleModel 卡片所在模块
     * @param targetName 卡片所在target
     * @param formPath 卡片相对路径
     * @return 完整路径
     */
    static resolveFormConfigPath(moduleModel: ModuleModel, targetName: string, formPath: string): string;
    /**
     * 返回传入ModuleModel的target下formWidgetModule指向的moduleModel对象
     *
     * @param moduleModel
     * @param targetName
     * @returns 返回ModuleModel，未找到则返回 undefined。
     */
    static getFormWidgetModuleByTargetName(moduleModel: ModuleModel, targetName: string): ModuleModel | undefined;
    /**
     * 返回传入ModuleModel的target下formExtensionModule指向的moduleModel对象
     *
     * @param moduleModel
     * @param targetName
     * @returns 返回ModuleModel，未找到则返回 undefined。
     */
    static getFormExtensionModuleByTargetName(moduleModel: ModuleModel, targetName: string): ModuleModel | undefined;
    /**
     * 卡片包模块将formExtensionModule中的form的extensionAbilities合并至moduleOptObj
     *
     * @param taskService
     * @param targetRes
     * @returns ModuleOptObj
     */
    static getTargetModuleOptObj(taskService: TargetTaskService, targetRes: ModuleTargetRes): ModuleOptObj;
}
