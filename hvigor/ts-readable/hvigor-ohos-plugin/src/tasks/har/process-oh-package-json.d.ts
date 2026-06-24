/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/process-oh-package-json.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhPackageJsonOpt, PackageJsonOpt } from '../task/ohos-har-task';
import { CoreProcessPackageJson } from './core-process-package-json';
/**
 * 闭源Har打包发布前需要对package.json文件进行处理和增加一些信息
 * 当前该任务执行处理很快,先不需要做增量
 */
export declare class ProcessOhPackageJson extends CoreProcessPackageJson {
    private readonly logger;
    private readonly generateOhPackageJsonFilePath;
    private readonly workerConfig;
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): void;
    protected addArtifactType(packageJsonObj: OhPackageJsonOpt): void;
    private processByteCodeHar;
    /**
     * 在打包har时，处理InterfaceHar字段
     * InterfaceHar字段对于har来说异常字段，会导致运行时crash，需要告警提示
     * @private
     */
    private processInterfaceHar;
    /**
     * 向oh-package.json5里添加worker信息
     *
     * @protected
     */
    protected addWorkerData(): void;
    /**
     * 处理worker信息
     *
     * @returns {string[] | undefined}
     * @private
     */
    private processWorkerConfig;
    /**
     * 构建har包时需要将dynamicImport写入har产物oh-package.json5的metadata中
     * 闭源har模块需要保证以相对路径形式导入的文件后缀为js格式写入
     * 包名,文件名,文件夹相对路径保持不变写入
     * @protected
     */
    protected addDynamicImport(): void;
}
/**
 * 如果有定制resource，则需要resource写入到oh-package.json的metadata中resource
 * 不支持绝对路径
 *
 * @private
 */
export declare function processResourceConfig(targetService: TargetTaskService, packageJsonObj: PackageJsonOpt | OhPackageJsonOpt): void;
