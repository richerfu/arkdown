/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-native-with-cmake.d.ts
 */
import { FileId } from '../utils/cmake/cmake-invalidation-state';
import { AbstractBuildNative } from './abstract-build-native';
import { TargetTaskService } from './service/target-task-service';
/**
 * ohos native代码编译任务
 *
 * @since 2021/1/21
 */
export declare class BuildNativeWithCmake extends AbstractBuildNative {
    private _log;
    private readonly _moduleDir;
    private static readonly sanAbilitySet;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    taskShouldDo(): boolean;
    protected doTaskAction(): Promise<void>;
    private ensureCommandFile;
    private getCmakeCallback;
    getInputFiles(dir: string, abi: string): Promise<FileId[]>;
    getOptionalOutputFiles(dir: string): FileId[];
    getRequiredOutputFiles(dir: string): FileId[];
    getHardOutputFiles(dir: string): FileId[];
    getToolchainFile(nativeCompiler: string | undefined, hmsAvailable: boolean, arch: string): string;
    buildCommand(abiFilter: string): string[];
    /**
     * 从hvigor参数设置中收集Cmake参数ohos-test-coverage,并返回
     *
     * @param testCoverage
     */
    getTestCoverageArg(testCoverage: string): string;
    /**
     * 是否为cppFlags和cFlags加“-iclang”参数
     *
     */
    isEnableCppIncrementalBuildArg(arch: string): boolean;
    /**
     * 为cppFlags和cFlags加“-iclang”参数
     *
     */
    addCppIncrementalBuildArg(flags: string): string;
    /**
     * 从hvigor参数设置和build-profile.json5配置文件中收集cmake参数，并返回
     * @param abiType 当前运行环境的abi类型
     */
    addNativeArgs(abiType: string): string[];
    /**
     * 检查当前sdk版本是否支持传入的sanitizer参数
     * @param cmakeArgs 一系列cmake参数字符串所构成的数组
     */
    checkSanArgsSupported(cmakeArgs: string[]): void;
    /**
     * 检查是否设置了多个相互排斥的san参数
     * @param cmakeArgs 一系列cmake参数字符串所构成的数组
     */
    checkExclusiveSan(cmakeArgs: string[]): void;
}
