/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/inner-java-command-builder/signing-options.d.ts
 */
import { JavaCommandBuilder } from '../java-command-builder';
export declare class SigningOptions extends JavaCommandBuilder {
    sign(): SigningOptions;
    addMode(mode: string): SigningOptions;
    addProfile(profile: string): SigningOptions;
    addSignAlg(signAlg: string): SigningOptions;
    addPrivateKey(privateKey: string): SigningOptions;
    profileSigned(): SigningOptions;
    addKeyStore(keyStoreFile: string): SigningOptions;
    addKeyStorePwd(keyStorePwd: string): SigningOptions;
    addKeyAliasPwd(keyAliasPwd: string): SigningOptions;
    addCertPath(certPath: string): SigningOptions;
    addKs(ks: string): SigningOptions;
    addKsPass(ksPass: string): SigningOptions;
    addKsKeyAlias(ksKeyAlias: string): SigningOptions;
    addKeyPass(keyPass: string): SigningOptions;
    addOut(out: string): SigningOptions;
    addInputFile(inputFile: string): SigningOptions;
    addOutputFile(outputFile: string): SigningOptions;
    add(param: string): SigningOptions;
}
