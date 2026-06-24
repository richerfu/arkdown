/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/find-target-product.d.ts
 */
import { ProjectModel } from '../model/project/project-model';
import { ProjectBuildProfile } from '../options/build/project-build-profile';
/**
 * 根据命令行中的配置找到根项目下的build-profile.json5中的Product
 *
 * @param projectModuleModel
 */
export declare function findTargetProduct(projectModuleModel: ProjectModel): ProjectBuildProfile.ProductBuildOpt;
