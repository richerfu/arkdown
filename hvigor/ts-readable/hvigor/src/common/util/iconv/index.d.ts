/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/iconv/index.d.ts
 */
/// <reference types="node" />
/// <reference types="node" />
import { Decoder, Encoder, Options } from './models';
declare class Iconv {
    private codecDataCache;
    /**
     * 使用指定字符集解码
     *
     * @param {Buffer | Uint8Array} buffer
     * @param {string} encoding
     * @param {Options} options
     * @return {string}
     */
    decode(buffer: Buffer | Uint8Array, encoding: string, options?: Options): string;
    /**
     * 使用指定字符集编码
     *
     * @param {string} content
     * @param {string} encoding
     * @param {Options} options
     * @return {Buffer}
     */
    encode(content: string, encoding: string, options?: Options): Buffer;
    /**
     * 判断字符集是否存在
     *
     * @param {string} encoding
     * @return {boolean}
     */
    encodingExists(encoding: string): boolean;
    /**
     * 获取指定字符集解码器
     *
     * @param {string} encoding
     * @param {Options} options
     * @return {Decoder}
     */
    getDecoder(encoding: string, options?: Options): Decoder;
    /**
     * 获取指定字符集编码器
     *
     * @param {string} encoding
     * @param {Options} options
     * @return {Encoder}
     */
    getEncoder(encoding: string, options?: Options): Encoder;
    private getCodec;
    private canonicalizeEncoding;
}
export declare const iconv: Iconv;
export {};
