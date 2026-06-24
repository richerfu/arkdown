/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/opt-compression-builder.d.ts
 */
import { RestoolCompressionConfig } from '../options/build/build-opt';
import { OptCompressionBuilderInterface } from './restool-builder-interface';
interface OptCompressionFilterItem {
    method: {
        type: 'astc' | 'sut';
        blocks: '4x4';
    };
    path?: string[];
    exclude_path?: string[];
    rules_origin?: {
        size?: number[][];
        resolution?: {
            width: number;
            height: number;
        }[][];
    };
    rules_exclude?: {
        size?: number[][];
        resolution?: {
            width: number;
            height: number;
        }[][];
    };
}
export interface OptCompression {
    context: {
        extensionPath: string;
    };
    compression: {
        media?: {
            enable: boolean;
        };
        sizeLimit?: {
            ratio: number;
        };
        filters: OptCompressionFilterItem[];
    };
}
export declare class OptCompressionBuilder implements OptCompressionBuilderInterface {
    private optCompressionObj;
    private allResourceFilesAbsolutePath;
    setResourceDirArr(resourceDirArr: string[]): OptCompressionBuilderInterface;
    setExtensionPath(extensionPath?: string): OptCompressionBuilderInterface;
    setCompressionConfig(compressionConfig: RestoolCompressionConfig | undefined): OptCompressionBuilderInterface;
    getOptCompressionObj(): OptCompression;
    /**
     * 处理区间列表，将所有字符串转换为数字，合并重叠区间
     * [ [0, "1k"], [1024, "2048"] ] --> [ [0, 2048] ]
     * @param size
     * @private
     */
    private processSize;
    /**
     * 校验 resolution 区间，过滤最小宽高大于最大宽高的无效 resolution，如下示例中第一个区间是有效的，第二个是无效的会被过滤掉
     * [
     *   [
     *     {width: 0, height: 0},
     *     {width: 100, height: 100},
     *   ],
     *   [
     *     {width: 100, height: 100},
     *     {width: 0, height: 0},
     *   ]
     * ]
     *
     * @param resolution
     * @private
     */
    private processResolution;
    /**
     * 从资源目录文件列表中筛选出与用户填的 path 匹配上的文件
     * @param globArr
     * @private
     */
    private getPathFromResourceFiles;
    /**
     * 转换 files 和 exclude 中的 size resolution
     * @param rules
     * @private
     */
    private convertRules;
    private convertFilters;
    /**
     * 假如 media===true && filters.length===0，需要加上一个默认的 filter
     * @private
     */
    private checkMediaFilter;
}
export {};
