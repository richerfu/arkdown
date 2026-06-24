/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/packing-check-util.d.ts
 */
import { TargetTaskService } from '../../tasks/service/target-task-service';
/**
 * 打包产物源码检查的工具类
 */
export declare class PackingCheckUtil {
    private static resourceFilesCache;
    /**
     * 获取自定义或者默认的resource目录
     */
    static getResourcePaths(targetService: TargetTaskService): string[];
    /**
     * 遍历文件夹，查找指定后缀的文件，同时需要返回命中条件的原始文件的相对路径
     * @param folderPath
     * @param filterFn
     */
    static collectSourceCodeFiles(folderPath: string, filterFn?: (file: string) => boolean): string[];
    /**
     * 传入相对路径集合和原始目录，还原出文件的原始路径
     * @param relativeFileList
     * @param originDir
     */
    static recoverOriginalPath(relativeFileList: string[], originDir: string): string[];
    /**
     * 传入相对路径集合和过滤后的中间文件路径，判断文件是否在最终打包产物中
     * @param relativeFileList
     * @param intermediateFileDir
     */
    static recoverPackagedFiles(relativeFileList: string[], intermediateFileDir: string): string[];
    static clearResourceFilesCache(): void;
}
