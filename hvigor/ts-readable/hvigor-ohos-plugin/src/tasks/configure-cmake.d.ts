/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/configure-cmake.d.ts
 */
import { ExternalNativeOpt } from '../options/build/build-opt';
import { Dependency } from '../project/dependency/core/dependency-interface';
import { ArchResolver, PackageResolver } from '../utils/native-cmake-resolver';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * 根据已安装依赖中是否存在native产物（libs/include）
 * 为依赖生成cmake find脚本(exampleConfig.cmake)
 * 并将find脚本路径传递给cmake, 方便开发者引用三方so及接口
 *
 * @since 2024/01/02
 */
export declare class ConfigureCmake extends OhosHapTask {
    protected readonly nativeOption?: ExternalNativeOpt;
    protected readonly dependencies: Dependency[];
    protected readonly cmakeDir: string;
    protected readonly abiList: string[];
    protected readonly packages: PackageResolver[];
    constructor(targetService: TargetTaskService);
    taskShouldDo(): boolean;
    init(): void;
    protected doTaskAction(): Promise<void>;
    writeCmakeScript(archResolvers: ArchResolver[]): Promise<void>;
    genCmakeFindFile(): Promise<void>;
    initTaskDepends(): void;
    private isLocal;
    private getModuleTarget;
    private isNativeLibrary;
}
