/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/keystore-utils.d.ts
 */
export declare class KeyStoreHelper {
    private static CERTIFICATE_DESC;
    private static DEFAULT_IDE_CONFIG_PATH;
    private static CONFIG;
    private static DEFAULT_DEBUG_SIGN_FILE;
    private static keyBitsLength;
    private static DEFAULT_STORE_PASS_FILE;
    static DEFAULT_STORE_PASS: string;
    static DEFAULT_ALIAS: string;
    private static validity;
    private static validFrom;
    private static keyAlgo;
    private static signatureAlgo;
    private static requestCertFile;
    private static generateCertFile;
    /**
     * 获得默认的密钥库地址
     */
    static getDefaultDebugKeyStoreLocation(): string | undefined;
    /**
     * 创建默认的密钥库
     */
    static createDefaultDebugStore(): void;
    private static createDebugStore;
    private static createNewStore;
    private static loadDefaultStorePass;
}
