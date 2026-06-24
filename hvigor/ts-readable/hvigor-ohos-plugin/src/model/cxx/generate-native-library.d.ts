/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/generate-native-library.d.ts
 */
import { NativeCMakeFiles } from './native-cmake-files';
import { NativeTarget } from './native-codemodel-target';
import { NativeLibraryModel } from './native-library-model';
export declare class GenerateNativeLibrary {
    static parseLibrary(abi: string, nativeTarget: NativeTarget, target: string, cmakeFiles?: NativeCMakeFiles): NativeLibraryModel;
    static findFolders(nativeTarget: NativeTarget): string[];
    private static findFiles;
    private static findOutputs;
    private static findRuntimeFiles;
    /**
     * 处理构建目录，将符合条件的文件路径添加到结果数组中。
     *
     * @param nativeTarget - 包含本地目标路径的对象。
     * @param cmakeFiles - 包含CMake文件路径的对象，可能为undefined。
     * @param runtimeFile - 需要在构建目录中查找的运行时文件名。
     * @param sysroot - 系统根目录路径，用于过滤文件路径，可能为undefined。
     * @param result - 存储符合条件的文件路径的结果数组。
     */
    private static handleBuildDir;
}
