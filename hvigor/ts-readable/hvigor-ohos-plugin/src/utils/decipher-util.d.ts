/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/decipher-util.d.ts
 */
export declare class DecipherUtil {
    private static component;
    private static dirs;
    private static _logger;
    private static macDSStore;
    private static materialDir;
    /**
     * 使用aes-gcm算法解密消息
     * AesGcmKit，其实现符合《密码算法应用规范 V1.4.docx》中针对于AES-GCM的使用要求
     * 填充参数是NoPadding，密钥长度16 byte(128 bit)，iv长度12 byte(96bit)，iv每次都是采用SecureRandom生成，tag参数长度16 byte(128 bit)。
     *
     * @param materialDir 签名材料地址
     * @param encryptedPwd 加密密码
     * @param signConfigSrcPath 配置签名的文件路径（build-profile.json5或hvigorfile.ts）
     * @return string
     */
    static decryptPwd(materialDir: string, encryptedPwd: string, signConfigSrcPath: string): string;
    /**
     * 获取解密密钥
     *
     * @param materialDir 签名材料地址
     * @param signConfigSrcPath
     * @private Int8Array
     */
    private static getKey;
    /**
     * PBKDF2WithHmacSHA256迭代10000次生成rootKey
     *
     * @param components
     * @param salt
     * @param signConfigSrcPath
     * @private
     */
    private static getRootKey;
    /**
     * 根据密钥解密, 获取明文密码
     * 被加密后的数据格式如下
     * 其中content数据中包含tag数据
     * -------------------------------------------------------------------------------
     * | content length (4byte) | iv key (12 byte) | encrypted | auth tag ( 16 byte ) |
     *  ------------------------------------------------------------------------------
     *
     * @param key 密钥
     * @param msg 加密密码
     * @param signConfigSrcPath
     * @private Buffer.Buffer
     */
    private static decrypt;
    private static xorComponents;
    /**
     * 异或
     *
     * @param b1
     * @param b2
     * @param signConfigSrcPath
     * @private
     */
    private static xor;
    /**
     * 读取./material/fd
     *
     * @param material
     * @param fd
     * @param signConfigSrcPath
     * @private
     */
    private static readFd;
    /**
     * 读取盐值./material/ac
     *
     * @param dir
     * @param signConfigSrcPath
     * @private
     */
    private static readSalt;
    /**
     * workMaterial ./material/ce
     *
     * @param dir
     * @param signConfigSrcPath
     * @private
     */
    private static readWorkMaterial;
    /**
     * 读取目录中文件 默认一个文件
     *
     * @param dir
     * @param signConfigSrcPath
     * @private
     */
    private static readDirBytes;
    /**
     * 读取默认的密钥库密码
     *
     * @param DEFAULT_STORE_PASS_FILE 存储加密后的密钥库密码的文件路径
     * @return string 文件中存储的加密的密码信息
     */
    static decryptPluginDataFromResource(DEFAULT_STORE_PASS_FILE: string): string;
    private static getProjectBuildProfilePath;
}
