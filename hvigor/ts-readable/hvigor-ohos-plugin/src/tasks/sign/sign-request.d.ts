/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/sign-request.d.ts
 */
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import SigningConfigBuildOpt = ProjectBuildProfile.SigningConfigBuildOpt;
export interface SignParams {
    mode: string;
    keyAlias: string;
    keyPwd: string;
    appCertFile: string;
    profileFile?: string;
    profileSigned?: number;
    inForm?: string;
    signAlg: string;
    keystoreFile: string;
    keystorePwd: string;
    signCode?: number;
    inFile: string;
    outFile: string;
}
export interface HarSignParams {
    keyAlias: string;
    keyPwd: string;
    signCertFile: string;
    profileFile?: string;
    inForm?: string;
    inFileForm?: string;
    signAlg: string;
    keystoreFile: string;
    keystorePwd: string;
    inFile: string;
}
export interface VerifyProfileParams {
    inFile: string;
    outFile: string;
}
export declare function sendSignRequest(signToolJarPath: string, signParams: SignParams): Promise<import("../java-daemon/websocket").SocketResp>;
export declare function sendSignHarRequest(signToolJarPath: string, signParams: HarSignParams): Promise<import("../java-daemon/websocket").SocketResp>;
export declare function sendVerifyProfileRequest(signToolJarPath: string, verifyProfileParams: VerifyProfileParams): Promise<import("../java-daemon/websocket").SocketResp>;
export declare function constructSignParams(signingConfig: SigningConfigBuildOpt, inFile: string, outFile: string, keystorePwd: string, keyPwd: string, inForm: string, mode?: string): SignParams;
export declare function constructHarSignParams(signingConfig: SigningConfigBuildOpt, inFile: string, outFile: string, keystorePwd: string, keyPwd: string): HarSignParams;
