/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/module-task-service-factory.d.ts
 */
import { ModuleModel } from '../../model/module/module-model';
import { ProjectModel } from '../../model/project/project-model';
import { ModuleTaskService } from './module-task-service';
/**
 * 用于创建每个module的ModuleTaskService
 *
 * @since 2022/8/11
 */
export declare class ModuleTaskServiceFactory {
    static initModuleTaskService(projectModel: ProjectModel, moduleModel: ModuleModel, isFaMode: boolean): ModuleTaskService;
}
