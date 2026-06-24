/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/iconv/bom-handling.d.ts
 */
/// <reference types="node" />
/// <reference types="node" />
import { Decoder, Encoder, Options } from './models';
/**
 * StripBOMDecoderWrapper 解码器
 */
export declare class StripBOMDecoderWrapper implements Decoder {
    private decoder;
    private pass;
    private options;
    constructor(decoder: Decoder, options?: Options);
    write(buffer: Buffer | Uint8Array): string;
    end(): string | undefined;
}
/**
 * PrependBOMEncoderWrapper 编码器
 */
export declare class PrependBOMEncoderWrapper implements Encoder {
    private readonly encoder;
    private addBOM;
    constructor(encoder: Encoder, options?: Options);
    write(str: string): Buffer;
    end(): Buffer | undefined;
}
