"use strict";

// The Rust stage owns configuration and diagnostics. This bridge mirrors the
// two SDK checker entries selected by Hvigor: CompileArkTS runs the SDK
// `etsChecker` plugin's buildStart hook, while ArkLint uses
// `runEtsStandaloneChecker`.

const fs = require("fs");
const path = require("path");
const util = require("util");

function render(value) {
  return typeof value === "string" ? value : util.format("%o", value);
}

function restoreMapFields(projectConfig) {
  for (const key of ["depName2OhExports", "depName2DepInfo", "depName2RootPath"]) {
    const value = projectConfig[key];
    if (value && typeof value === "object" && !(value instanceof Map)) {
      projectConfig[key] = new Map(
        Object.entries(value).map(([name, info]) => [name, info]),
      );
    }
  }
}

// hvigor-arkts-compose/load-sdk-functions.js::hijackTscCompilerOptionsPaths.
// The SDK's service checker augments the wildcard path entry after this
// property is installed, preserving package-specific paths from Hvigor.
function hijackTscCompilerOptionsPaths(etsLoaderPath, otherPaths) {
  const checkerPath = path.join(etsLoaderPath, "lib", "ets_checker.js");
  if (!fs.existsSync(checkerPath)) {
    throw new Error(`SDK etsChecker implementation is unavailable: ${checkerPath}`);
  }
  const compilerOptions = require(checkerPath).compilerOptions;
  if (!compilerOptions) {
    return;
  }
  const descriptor = Object.getOwnPropertyDescriptor(compilerOptions, "paths");
  if (descriptor && !descriptor.configurable) {
    return;
  }
  let paths = otherPaths;
  Object.defineProperty(compilerOptions, "paths", {
    enumerable: true,
    configurable: true,
    get: () => paths,
    set(value) {
      if (paths && Object.prototype.hasOwnProperty.call(paths, "*") &&
          value && Object.prototype.hasOwnProperty.call(value, "*")) {
        paths["*"] = Array.from(new Set([...paths["*"], ...value["*"]]));
        delete value["*"];
      }
      paths = { ...paths, ...value };
    },
  });
}

function runCompileChecker(sdk, request, projectConfig, logger) {
  if (typeof sdk.initConfig !== "function" || typeof sdk.sdkPlugins !== "function") {
    throw new Error(`SDK CompileArkTS checker plugins are unavailable: ${request.etsLoaderPath}`);
  }
  hijackTscCompilerOptionsPaths(request.etsLoaderPath, projectConfig.otherPaths);
  sdk.initConfig();
  const checker = sdk.sdkPlugins(projectConfig).find((plugin) => plugin?.name === "etsChecker");
  if (!checker || typeof checker.buildStart !== "function") {
    throw new Error(`SDK etsChecker buildStart hook is unavailable: ${request.etsLoaderPath}`);
  }
  const cache = new Map();
  const share = {
    projectConfig,
    cache,
    depInfo: { enableIncre: false, changedDeps: [] },
    getLogger: () => logger,
  };
  checker.buildStart.call({ share, cache });
}

function runStandaloneChecker(sdk, request, projectConfig, logger) {
  if (typeof sdk.etsStandaloneChecker !== "function") {
    throw new Error(`SDK etsStandaloneChecker is unavailable: ${request.etsLoaderPath}`);
  }
  sdk.etsStandaloneChecker(request.entries, logger, projectConfig);
}

function main() {
  const requestPath = process.argv[2];
  const responsePath = process.argv[3];
  if (!requestPath || !responsePath) {
    throw new Error("arkdown etsChecker bridge requires request and response paths");
  }
  const request = JSON.parse(fs.readFileSync(requestPath, "utf8"));
  for (const [key, value] of Object.entries(request.loaderEnv || {})) {
    if (value === null || value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = String(value);
    }
  }
  // This is intentionally the same platform compatibility assignment that
  // latest hvigor's generateLoaderEnv applies before loading ets-loader.
  process.env.path = process.env.Path;
  process.chdir(request.workingDirectory);

  const diagnostics = [];
  const capture = (level, args) => {
    diagnostics.push({ level, message: args.map(render).join(" ").trim() });
  };
  const logger = new Proxy(
    {
      error: (...args) => capture("error", args),
      warn: (...args) => capture("warn", args),
      info: (...args) => capture("info", args),
      debug: (...args) => capture("debug", args),
    },
    {
      get(target, property) {
        if (property in target) {
          return target[property];
        }
        return (...args) => capture("debug", args);
      },
    },
  );
  const sdk = require(path.join(request.etsLoaderPath, "compile_plugin.js"));
  // The SDK checker expects `depName2OhExports`/`depName2DepInfo` to be Maps
  // (`isOhExport` calls `?.has(pkgName)`); JSON transport delivers plain
  // objects, so rebuild the Map-shaped fields before the checker runs.
  const projectConfig = { ...request.projectConfig };
  restoreMapFields(projectConfig);
  if (request.phase === "compile") {
    runCompileChecker(sdk, request, projectConfig, logger);
  } else if (request.phase === "standalone") {
    runStandaloneChecker(sdk, request, projectConfig, logger);
  } else {
    throw new Error(`unsupported SDK etsChecker phase: ${request.phase}`);
  }
  const checkerExitCode = process.exitCode || 0;
  // Diagnostic severity is returned to Rust. Avoid a process-level exit that
  // would discard the complete deterministic diagnostic list.
  process.exitCode = 0;
  fs.writeFileSync(
    responsePath,
    JSON.stringify({ diagnostics, checkerExitCode }),
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error && error.stack ? error.stack : error}\n`);
  process.exitCode = 1;
}
