/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/native-command-builder.d.ts
 */
import { BaseCommandBuilder } from './base-command-builder';
import { OhosLogger } from '../utils/log/ohos-logger';
/**
 * ohos native代码编译命令行构造类
 *
 * @since 2022/1/21
 */
export declare class NativeCommandBuilder extends BaseCommandBuilder {
    protected log: OhosLogger;
    constructor(executor: string);
    addCmakeList(cmakeList: string): NativeCommandBuilder;
    addTempFilePath(tempFilePath: string): NativeCommandBuilder;
    addOhosArch(arch: string): NativeCommandBuilder;
    /**
     * 不要删除宏 CMAKE_LIBRARY_OUTPUT_DIRECTORY
     * 稳定作为cmake可用环境变量
     *
     * @param outputDir 库输出目录
     */
    addOutputDir(outputDir: string): NativeCommandBuilder;
    addCMakeFindPath(findPath: string): this;
    addPackageFindScript(script: string): this;
    addBuildType(buildType: string): NativeCommandBuilder;
    addNativeSdk(nativeSdk: string): NativeCommandBuilder;
    addHMosNativeSdk(nativeSdk: string): NativeCommandBuilder;
    addSystemName(systemName: string): NativeCommandBuilder;
    addOhosArchAbi(archAbi: string): NativeCommandBuilder;
    exportCompileCommands(exportCommand: string): NativeCommandBuilder;
    addToolChain(toolchain: string): NativeCommandBuilder;
    addGenerator(generator: string): NativeCommandBuilder;
    addMakeProgramPath(makeProgramPath: string): NativeCommandBuilder;
    addCFlags(cFlags: string): NativeCommandBuilder;
    addCxxFlags(cppFlags: string): NativeCommandBuilder;
    addOhosCoverage(ohosCoverage: string): NativeCommandBuilder;
    changeToDir(dir: string): NativeCommandBuilder;
    muteUnusedCliWarn(): NativeCommandBuilder;
}
