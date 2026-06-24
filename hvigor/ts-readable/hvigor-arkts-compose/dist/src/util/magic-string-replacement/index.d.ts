/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/magic-string-replacement/index.d.ts
 */
export declare class MagicStringReplacement {
    private prefix;
    private readonly sourceCode;
    private suffix;
    /**
     * 以链表形式存放代码片段
     */
    private listHead;
    /**
     * 根据在源代码中的起始字符下标获取对应的代码片段，左闭区间
     */
    private fragmentAtStartIndex;
    /**
     * 根据在源代码中的末尾字符下标获取对应的代码片段，右开区间
     */
    private fragmentAtEndIndex;
    constructor(sourceCode: string);
    /**
     * @returns [prefix] + [prefix,frag1,suffix] + [prefix,frag2,suffix] + [suffix]
     */
    toString(): string;
    /**
     * 在源代码的末尾继续拼接代码
     * @param appendCode
     */
    append(appendCode: string): void;
    /**
     * 将代码插入到 index 所在代码片段后缀的右侧
     * 注意假如 index 不在源代码的范围内，则插入到完整源代码前缀的右侧
     * @param index
     * @param insertCode
     */
    appendLeft(index: number, insertCode: string): void;
    /**
     * remove(3,7)，入参左闭右开，先分割片段，再将范围内的片段内容重写为空字符串
     * [01234][56789]
     *        ↓
     * [012][34][56][789]
     *        ↓
     * [012][][][789]
     *
     * @param start
     * @param end
     * @returns
     */
    remove(start: number, end: number): void;
    /**
     * overwrite(3,7,'abc')，入参左闭右开，先分割片段，再将内容写入到范围内的第一个片段，其余片段置空
     * [01234][56789]
     *        ↓
     * [012][34][56][789]
     *        ↓
     * [012][abc][][789]
     *
     * @param start
     * @param end
     * @param content
     */
    overwrite(start: number, end: number, content: string): void;
    generateMap(option: {
        hires: true;
    }): {
        mappings: string;
        version: number;
        names: never[];
        file: null;
        sourcesContent: null[];
        sources: null[];
        toString(): string;
        toUrl(): string;
    };
    /**
     * 根据在源代码中的下标进行分割
     * @param sourceIndex
     * @returns
     */
    private splitFragment;
}
