/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/restool-command-builder.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { RestoolLinkParamObj } from '../options/configure/restool-config-options';
import { BaseCommandBuilder } from './base-command-builder';
import { RestoolBuilderInterface } from './restool-builder-interface';
export declare class RestoolCommandBuilder extends BaseCommandBuilder implements RestoolBuilderInterface {
    inputFiles: FileSet;
    outputFiles: FileSet;
    existsResourceDir: boolean;
    restoolLinkParamObj: RestoolLinkParamObj;
    constructor(executor: string);
    addInputVariantDir(inputVariantDir: string): RestoolBuilderInterface;
    addInputDir(inputDir: string, inputType?: string): RestoolBuilderInterface;
    addEntryCompiledResource(inputDir: string): RestoolBuilderInterface;
    addAppScopeResourcesDir(inputDir: string): RestoolBuilderInterface;
    addModuleResourcesMap(map: Map<string, string>): RestoolBuilderInterface;
    addHarResourcesMap(map: Map<string, string>): RestoolBuilderInterface;
    addJsonFile(jsonFile: string): RestoolBuilderInterface;
    addModulePackName(packName: string): RestoolBuilderInterface;
    addOutputDir(outputDir: string): RestoolBuilderInterface;
    addIdsPath(idsPath: string): RestoolBuilderInterface;
    addResTable(resTable: string): RestoolBuilderInterface;
    forceDelete(): RestoolBuilderInterface;
    incremental(): RestoolBuilderInterface;
    compressImage(): RestoolBuilderInterface;
    addOutputBak(outputBak: string): RestoolBuilderInterface;
    addModules(modules: string): RestoolBuilderInterface;
    addStartId(startId: string): RestoolBuilderInterface;
    setIconCheck(enable: boolean): RestoolBuilderInterface;
    incrementalCompile(): RestoolBuilderInterface;
    addDefinedIds(idsPath: string): this;
    addDefinedSysIds(definedIds: string): this;
}
