/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/module-dependency-util.d.ts
 */
import { ModuleModel } from '../model/module/module-model';
import { ProjectModel } from '../model/project/project-model';
import { TaskService } from '../tasks/service/task-service';
/**
 * 记录Project工程中的entry依赖的本地har的情况下，har的依赖链中只存在har 不存在hsp
 * 此时的har合并到entry的时候不需要生成resource_str目录
 */
declare class ModuleDependencyUtil {
    private pureHar;
    getPureHar(service: TaskService, module: ModuleModel, projectModel: ProjectModel, isFaMode: boolean): Set<string>;
    init(service: TaskService, module: ModuleModel, projectModel: ProjectModel, isFaMode: boolean): void;
    /**
     * 收集纯HAR模块。
     *
     * @param projectModel - 项目模型对象，包含项目的模块信息。
     * @param dependency - 依赖对象，表示需要检查的依赖项。
     * @param isFaMode - 布尔值，指示是否处于FA模式。
     */
    private collectPureHar;
}
export declare const moduleDependencyUtil: ModuleDependencyUtil;
export {};
