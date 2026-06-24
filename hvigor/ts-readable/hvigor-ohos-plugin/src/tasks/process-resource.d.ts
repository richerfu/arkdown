/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-resource.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { OptCompressionBuilder } from '../builder/opt-compression-builder';
import { RestoolConfigBuilder } from '../builder/restool-file-config-builder';
import { AbstractProcessResource } from './abstract/abstract-process-resource';
import { TargetTaskService } from './service/target-task-service';
/**
 * Stage 模型用于处理和生成用文件方式编译资源的中间文件
 *
 * @since 2022/9/8
 */
export declare class ProcessResource extends AbstractProcessResource {
    private readonly _log;
    protected optCompressionBuilder: OptCompressionBuilder;
    protected optCompressionFilePath: string;
    private resourcesDuplicate;
    private allHspDepTargets;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    protected beforeTask(): void;
    protected getRestoolConfigBuilder(): RestoolConfigBuilder;
    declareInputs(): Map<string, import("@ohos/hvigor").TaskInputValue>;
    private get ignoreResourcePattern();
    private get qualifiersConfig();
    protected doTaskAction(): void;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    private getAppResourceDir;
    private get includeAppScopeRes();
    initCommandBuilder(): void;
    getOptCompressionBuilder(): OptCompressionBuilder;
    /**
     * har包去重场景：编译hap和hsp时如果有用户自定义的全局资源id固定表以用户自定义的表为准
     */
    private processCustomIdDefinedFile;
    private shouldDoHarDeduplicate;
    private writeHspDeduplicateResConfig;
    /**
     * 编译hap时把hsp搜集到的去重资源文件作为输入
     * 要搜集直接依赖和间接依赖的所有本地hsp
     * @param inputFiles 输入文件
     */
    private processHarDeduplicateInputFiles;
    /**
     * har包资源去重
     * @param restoolConfigBuilder RestoolConfigBuilder
     */
    private processHarResourceDeduplicate;
    /**
     * 编译hsp时搜集要去重har包的资源，之后要添加到entry hap中。
     * @param resDependencies 依赖的har包资源路径
     */
    private collectDuplicateHarResources;
    /**
     * 把搜集到的需要去重的资源写入临时文件
     * @private
     */
    private writeDuplicateResourcesToIntermediateFile;
    /**
     * 编译hap时添加去重har的资源
     * @param restoolConfigBuilder RestoolConfigBuilder
     */
    private addDuplicateHarResources;
}
