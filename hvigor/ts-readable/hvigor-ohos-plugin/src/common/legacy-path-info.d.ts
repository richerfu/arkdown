/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/legacy-path-info.d.ts
 */
export interface LegacyPathInfo {
    /**
     * 构建过程中merge后的老config.json文件的目录
     *
     * @return string
     */
    getIntermediatesMergeLegacyProfile: () => string;
    /**
     * 构建处理老config.json后的中间目录
     *
     * @return string
     */
    getIntermediatesProcessLegacyProfile: () => string;
    /**
     * 获取js编译manifest.json的目录
     *
     * @return string
     */
    getIntermediatesLegacyManifestJson: () => string;
    /**
     * 获取瘦设备生成bin资源的目录
     *
     * @return string
     */
    getIntermediatesLiteBinSource: () => string;
    /**
     * 构建处理resources后生成ResourceTable.h的目录
     *
     * @return string
     */
    getGenerateSourceR: () => string;
    /**
     * 获取模块级别的pack.info父目录
     */
    getIntermediatesPackInfoDir: () => string;
    /**
     * 构建过程中c++代码编译输出路径 entry/.cxx
     *
     * @return string
     */
    getCppOutputDir: () => string;
    /**
     * 构建过程中c++代码编译输出目录
     *
     * @return ../build/intermediates/cmake/[target]/obj
     */
    getIntermediatesCppOutPut: () => string;
    /**
     * 生成的java代码目录
     *
     * @return {string} ../build/intermediates/java/[target]
     */
    getIntermediatesJavaDir: () => string;
    /**
     * 生成的shell java代码目录
     *
     * @return {string} ../build/intermediates/shell/[target]/src/main/java/
     */
    getIntermediatesShellJavaDir: () => string;
    /**
     * 生成的shell class代码目录
     *
     * @return {string} ../build/intermediates/shell/[target]/build/classes
     */
    getIntermediatesShellClassDir: () => string;
    /**
     * 生成的class文件目录
     *
     * @return {string} ../build/intermediates/class/[target]
     */
    getIntermediatesClassDir: () => string;
    /**
     * 生成的dex目录
     *
     * @return {string} ../build/intermediates/dex/[target]
     */
    getIntermediatesDexDir: () => string;
    /**
     * 生成的dex目录
     *
     * @return {string} ../build/intermediates/shell/[target]/build/dex
     */
    getIntermediatesShellDexDir: () => string;
    /**
     * 生成的shell目录
     *
     * @return {string} ../build/intermediates/shell/[target]/src/main/
     */
    getShellSrcMain: () => string;
    /**
     * 构建过程中shell工程的res资源目录
     *
     * @return ../build/intermediates/shell/[target]/src/main/res
     */
    getShellResourceDir: () => string;
}
