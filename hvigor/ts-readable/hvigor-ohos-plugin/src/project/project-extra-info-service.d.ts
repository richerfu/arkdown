/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/project-extra-info-service.d.ts
 */
import { RuntimeTypeEnum } from '../enum/runtime-type-enum';
import { ProjectModel } from '../model/project/project-model';
/**
 * 提供一个单例对象，来初始化工程的所有模块的额外的状态信息，并在执行任务时获取
 *
 * @since 2022/1/18
 */
export declare class ProjectExtraInfoService {
    static getProjectRuntimeOS(project: ProjectModel): RuntimeTypeEnum;
}
