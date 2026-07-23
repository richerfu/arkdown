"use strict";

const fs = require("fs");
const path = require("path");

function fail(message) {
  throw new Error(message);
}

function logger(message, _details, level) {
  if (level === "error") {
    fail(message);
  }
}

async function main() {
  const requestPath = process.argv[2];
  const responsePath = process.argv[3];
  if (!requestPath || !responsePath) {
    fail("arkguard bridge requires request and response paths");
  }
  const request = JSON.parse(fs.readFileSync(requestPath, "utf8"));
  // ArkGuard's collectReservedNameForObf transformer follows the same loader
  // mode gate used by ets-loader's Rolldown plugin.
  process.env.compileTool = "rollup";
  process.env.compileMode = "esmodule";
  process.env.cachePath = request.cachePath;
  const arkguard = require(request.arkguardPath);
  const configResolver = require(path.join(
    request.arkguardPath,
    "lib/initialization/ConfigResolver.js",
  ));
  const projectConfig = {
    modulePath: request.moduleRoot,
    projectTopDir: request.projectRoot,
    projectRootPath: request.projectRoot,
    projectPath: request.projectPath,
    cachePath: request.cachePath,
    aceModuleBuild: request.aceModuleBuild,
    aceModuleJsonPath: request.aceModuleJsonPath,
    compileHar: request.compileHar,
    compileShared: request.compileShared,
    byteCodeHar: false,
    obfuscationOptions: {
      selfConfig: {
        ruleOptions: {
          enable: true,
          rules: request.ruleFiles,
        },
        consumerRules: [],
      },
      dependencies: {
        libraries: [],
        hspLibraries: [],
        hars: request.dependencyRuleFiles,
        hsps: [],
      },
      sdkApis: request.sdkApis,
      obfuscationCacheDir: request.cacheDir,
      exportRulePath: request.exportRulePath,
    },
  };
  const arkProjectConfig = {
    allowEtsAnnotations: true,
    entryArrayForObf: request.entryArray,
    modulePathMap: request.modulePathMap,
  };
  arkguard.initObfuscationConfig(projectConfig, arkProjectConfig, logger);
  const obfuscator = arkProjectConfig.arkObfuscator;
  const merged = arkProjectConfig.obfuscationMergedObConfig;
  if (!obfuscator || !merged || merged.options.disableObfuscation) {
    fail("ArkGuard did not initialize an enabled source obfuscator");
  }

  const scanOptions = {
    mNameObfuscation: {
      mEnable: true,
      mReservedProperties: [],
      mRenameProperties: merged.options.enablePropertyObfuscation,
      mKeepStringProperty: !merged.options.enableStringPropertyObfuscation,
      mEnableAtKeep: merged.options.enableAtKeep,
    },
    mExportObfuscation: merged.options.enableExportObfuscation,
    mKeepFileSourceCode: {
      mKeepSourceOfPaths: new Set(),
      mkeepFilesAndDependencies: new Set(),
    },
  };
  const originalSourcePaths = new Set(
    request.files.map((file) => file.originalPath),
  );
  const reserved = arkguard.readProjectPropertiesByCollectedPaths(
    originalSourcePaths,
    scanOptions,
    request.compileHar,
    false,
  );
  if (merged.options.enablePropertyObfuscation) {
    obfuscator.addReservedSetForPropertyObf(reserved);
  }
  if (merged.options.enableExportObfuscation) {
    obfuscator.addReservedSetForDefaultObf(reserved);
  }

  // ets-loader registers this ArkGuard transformer after the ArkUI syntax
  // transform. It discovers ViewPU members generated from ArkTS structs (for
  // example state backing fields) without maintaining a second name list.
  const typescript = require(require.resolve("typescript", {
    paths: [request.arkguardPath],
  }));
  const generatedSources = new Map();
  for (const file of request.files) {
    const content = fs.readFileSync(file.buildPath, "utf8");
    generatedSources.set(file.buildPath, content);
    const sourceFile = typescript.createSourceFile(
      file.buildPath,
      content,
      typescript.ScriptTarget.Latest,
      true,
      typescript.ScriptKind.TS,
    );
    const transformed = typescript.transform(sourceFile, [
      arkguard.collectReservedNameForObf(merged, false),
    ]);
    transformed.dispose();
  }

  const projectInfo = {
    packageDir: request.packageDir,
    projectRootPath: request.projectRoot,
    localPackageSet: new Set(request.localPackages),
    useNormalized: request.useNormalized,
    useTsHar: request.useTsHar,
  };
  const sourceMaps = {};
  const outputFiles = [];
  for (const file of request.files) {
    const content = generatedSources.get(file.buildPath);
    const historyNameCache = new Map();
    const previousCache = arkguard.nameCacheMap.get(file.relativePath);
    if (previousCache && previousCache.IdentifierCache) {
      arkguard.deleteLineInfoForNameString(
        historyNameCache,
        previousCache.IdentifierCache,
      );
    }
    const result = await obfuscator.obfuscate(
      content,
      {
        buildFilePath: file.buildPath,
        relativeFilePath: file.relativePath,
      },
      file.previousSourceMap,
      historyNameCache,
      file.originalPath,
      projectInfo,
    );
    if (typeof result.content !== "string") {
      fail(`ArkGuard produced no source for ${file.relativePath}`);
    }
    fs.writeFileSync(file.buildPath, result.content);
    if (result.sourceMap) {
      result.sourceMap.sources = [file.relativePath];
      result.sourceMap["entry-package-info"] = request.entryPackageInfo;
      sourceMaps[file.sourceMapKey] = result.sourceMap;
    }
    if (result.nameCache) {
      result.nameCache.obfName = file.relativePath;
      arkguard.nameCacheMap.set(file.relativePath, result.nameCache);
    }
    outputFiles.push({
      inputPath: file.buildPath,
      outputPath: file.buildPath,
    });
  }

  const nameCache = configResolver.getArkguardNameCache(
    merged.options.enablePropertyObfuscation,
    merged.options.enableFileNameObfuscation,
    merged.options.enableExportObfuscation,
    request.compileSdkVersion,
    request.entryPackageInfo,
  );
  fs.mkdirSync(request.cacheDir, { recursive: true });
  const nameCachePath = path.join(request.cacheDir, "nameCache.json");
  fs.writeFileSync(nameCachePath, nameCache);
  fs.writeFileSync(
    responsePath,
    JSON.stringify({ sourceMaps, nameCachePath, outputFiles }),
  );
  arkguard.clearGlobalCaches();
}

main().catch((error) => {
  process.stderr.write(`${error && error.stack ? error.stack : error}\n`);
  process.exitCode = 1;
});
