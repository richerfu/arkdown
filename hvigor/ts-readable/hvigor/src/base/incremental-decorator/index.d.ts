/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/index.d.ts
 */
export declare const Input: () => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
/**
 * 增量输入声明装饰器，用于声明外部的增量输入，仅支持Map：
 * class A extends IncrementalTask {
 *   @Inputs()
 *   private Map<string, TaskInputValue> = new Map();
 *
 * }
 */
export declare const Inputs: () => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
/**
 * 增量输入声明装饰器，用于声明外部的增量输入：文件/文件夹路径，示例：
 * class A extends IncrementalTask {
 *   @InputFile({ isDirectory: false })
 *   private string jsonPath = '/a/b.json';
 *
 *   @InputFile({ isDirectory: false })
 *   private get ohPackageJsonPath() {
 *     return '/a/b.json';
 *   }
 * }
 */
export declare const InputFile: (options?: import("../internal/snapshot/util/file-set").FsOptions | undefined) => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
/**
 * 增量输入声明装饰器，用于声明外部的增量输入：文件/文件夹路径数组，示例：
 * class A extends IncrementalTask {
 *   @InputFiles({ isDirectory: false })
 *   private string jsonPaths = ['/a/b.json'];
 *
 *   @InputFiles({ isDirectory: false })
 *   private get ohPackageJsonPath() {
 *     return ['/a/b.json'];
 *   }
 * }
 */
export declare const InputFiles: (options?: import("../internal/snapshot/util/file-set").FsOptions | undefined) => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
/**
 * 增量输出声明装饰器，用于声明任务的输出：文件/文件夹路径，示例：
 * class A extends IncrementalTask {
 *   @OutputFile({ isDirectory: false })
 *   private string jsonPaths = '/a/b.json';
 *
 *   @OutputFile({ isDirectory: false })
 *   private get ohPackageJsonPath() {
 *     return '/a/b.json';
 *   }
 * }
 */
export declare const OutputFile: (options?: import("../internal/snapshot/util/file-set").FsOptions | undefined) => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
/**
 * 增量输出声明装饰器，用于声明任务的输出：文件/文件夹路径数组，示例：
 * class A extends IncrementalTask {
 *   @OutputFiles({ isDirectory: false })
 *   private string jsonPaths = ['/a/b.json'];
 *
 *   @OutputFile({ isDirectory: false })
 *   private get ohPackageJsonPath() {
 *     return ['/a/b.json'];
 *   }
 * }
 */
export declare const OutputFiles: (options?: import("../internal/snapshot/util/file-set").FsOptions | undefined) => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
/**
 * 用于装饰 declareInputs、declareOutputFiles、declareInputFiles
 * 如果使用了该装饰器，也就是告知了hvigor，增量输入/输出只收集到当前对象，不再向上收集
 */
export declare const IgnoreSuperIncremental: () => (target: import("../external/task/incremental-task").IncrementalTask, propertyKey: string) => void;
