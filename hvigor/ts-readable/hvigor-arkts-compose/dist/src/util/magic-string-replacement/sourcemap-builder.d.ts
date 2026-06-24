/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/magic-string-replacement/sourcemap-builder.d.ts
 */
import { CodeFragment } from './code-fragment';
/**
 * [
 *     [ [number,0,number,number] ],
 *     [ [number,0,number,number], [number,0,number,number] ], // 对应生成代码一行中的每一列
 * ]
 *
 * 每一列的 Mapping
 * [
 *     number, // 结果代码的列
 *     0,      // 表示源代码所在的文件，没有用于多个源码文件打包的场景所以此处都填 0
 *     number, // 源代码的行
 *     number, // 源代码的列
 * ]
 */
type DecodedMappings = Array<Array<[number, 0, number, number]>>;
export declare class SourceMapBuilder {
    curGeneratedLine: number;
    curGeneratedColumn: number;
    decodedMappings: DecodedMappings;
    sourceCode: string;
    splitSourceCode: string[];
    firstCodeFragment: CodeFragment;
    sourceCodePrefix: string;
    constructor(sourceCode: string, firstCodeFragment: CodeFragment, sourceCodePrefix: string);
    /**
     * 生成 source map，只有 mappings 属性有实际数据
     */
    generateMap(): {
        mappings: string;
        version: number;
        names: never[];
        file: null;
        sourcesContent: null[];
        sources: null[];
        toString(): string;
        toUrl(): string;
    };
    private resolveCodeFragment;
    private offsetToLineColumn;
    /**
     * 根据 content 长度移动 curGeneratedLine 和 curGeneratedColumn
     */
    private moveLineColumnPointer;
}
export {};
