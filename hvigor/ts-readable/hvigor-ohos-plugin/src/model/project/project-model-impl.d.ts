/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/project-model-impl.d.ts
 */
import { Project } from '@ohos/hvigor';
import { AppRes } from '../res/res-model';
import { CoreProjectModelImpl } from './core-project-model-impl';
import { StageProjectAppRes } from './project-model';
/**
 * Stage模型的工程持久化数据模型,包含工程源码数据,配置数据等
 *
 * @since 2022/1/20
 */
export declare class ProjectModelImpl extends CoreProjectModelImpl implements StageProjectAppRes {
    private readonly appRes;
    constructor(project: Project);
    initSubModules(): void;
    /**
     * 获取该工程默认的bundleName值,即AppScope中配置的值
     */
    getDefaultBundleName(): string;
    getBundleType(): string;
    getAppRes(): AppRes;
}
