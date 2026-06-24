/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-libs.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { Dependency } from '../project/dependency/core/dependency-interface';
import { ProcessLibsCommonProperties } from './common-properties/process-libs-common-properties';
import { TargetTaskService } from './service/target-task-service';
/**
 * 收集hap和har依赖中的.so文件
 *
 * @since 2022/1/20
 */
export declare class ProcessLibs extends ProcessLibsCommonProperties {
    private readonly logger;
    private localOutput;
    private libsOutputDir;
    private _nativeOption?;
    private filterRule;
    private allHarLibs;
    private libsWithPkg;
    private hasLocalNativeOutput;
    private collectAllLibs;
    private readElf;
    private compileCommandsPath;
    private cangjieLibs;
    private localOutputHash?;
    private isHarType;
    private nativeOptionArg;
    private nativeLibs;
    private pkgsDuplicate;
    taskShouldDo(): boolean;
    beforeAlwaysAction(): Promise<void>;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    constructor(taskService: TargetTaskService);
    /**
     * 由于再构造函数中执行逻辑如果此任务的对象被提前实例化会导致某些意外现象，例如遗漏了某些so未收集
     * 此处就将构造函数中的初始化变量转移到init中
     * @private
     */
    private init;
    get isBundledDependencies(): boolean;
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
    private getHarLibsWithPkg;
    private getAllHarLibs;
    /**
     * 收集模块本地依赖har模块的so
     * @param depTargetService
     * @param pkgs
     * @private
     */
    private collectLocalHarLibs;
    /**
     * 收集模块远程依赖har模块的so
     * 若har模块bundledDependencies为true时不收集其依赖字节码har的so
     * @param targetService
     * @param pkgs
     * @private
     */
    private collectRemoteHarLibs;
    private collectDuplicateHar;
    private collectDuplicateHarForHap;
    excludeDuplicateHarForHsp(pkgs: Map<string, Dependency>): void;
    /**
     * 记录要去重的so到中间文件
     */
    private writeDuplicatePkgsToIntermediateFile;
    /**
     * 处理har包去重输入
     */
    private processHarDeduplicateInputFiles;
}
