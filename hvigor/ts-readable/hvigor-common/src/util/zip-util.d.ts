/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/zip-util.d.ts
 */
import zip from '@zip.js/zip';
export declare function getZipReader(zipFilePath: Blob): zip.ZipReader<unknown>;
export declare function getZipWriter(zipFilePath: string): zip.ZipWriter<Blob>;
export declare function getUint8ArrayReader(filePath: string): zip.Uint8ArrayReader;
export declare function addFilesToZip(zipWriter: zip.ZipWriter<Blob>, basePath: string, visitedPaths: Set<string>, relativePath?: string): Promise<void>;
/**
 * 读取zip包根目录的文件信息
 *
 * @param zipPath zip包路径
 * @param fileList  需要读取的文件列表
 */
export declare function readFileInZIP(zipPath: string, fileList: string[]): Promise<Map<string, string>>;
