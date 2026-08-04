"use strict";

// The Rust stage owns configuration and diagnostics. This bridge mirrors only
// hvigor-ohos-plugin/tasks/worker/util.js::generateLoaderEnv and
// hvigor-arkts-compose/load-sdk-functions.js::runEtsStandaloneChecker.

const fs = require("fs");
const path = require("path");
const util = require("util");

function render(value) {
  return typeof value === "string" ? value : util.format("%o", value);
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
  if (typeof sdk.etsStandaloneChecker !== "function") {
    throw new Error(`SDK etsStandaloneChecker is unavailable: ${request.etsLoaderPath}`);
  }
  sdk.etsStandaloneChecker(request.entries, logger, request.projectConfig);
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
