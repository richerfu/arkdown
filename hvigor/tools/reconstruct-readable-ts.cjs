#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..", "..");
const defaultSourceRoot =
  "/Users/ranger/.meat/ide/DevEco-Studio-6.1.1.280.app/Contents/tools/hvigor";
const sourceRoot = path.resolve(process.env.HVIGOR_SOURCE_ROOT || defaultSourceRoot);
const outputRoot = path.join(repoRoot, "hvigor", "ts-readable");
const babelRoot =
  "/Users/ranger/.meat/ide/DevEco-Studio-6.1.1.280.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules";

const parser = require(path.join(babelRoot, "@babel/parser"));
const traverse = require(path.join(babelRoot, "@babel/traverse")).default;
const generate = require(path.join(babelRoot, "@babel/generator")).default;
const t = require(path.join(babelRoot, "@babel/types"));

const helperNames = new Set([
  "__createBinding",
  "__setModuleDefault",
  "__importStar",
  "__importDefault",
  "__exportStar",
]);

const packageSpecs = [
  {
    name: "hvigor",
    root: "hvigor",
    includeDirs: ["bin", "src"],
    includeFiles: ["index.js"],
  },
  {
    name: "hvigor-ohos-plugin",
    root: "hvigor-ohos-plugin",
    includeDirs: ["src"],
    includeFiles: ["index.js"],
  },
  {
    name: "hvigor-arkts-base",
    root: "hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base",
    includeDirs: ["dist"],
    includeFiles: ["index.js"],
  },
  {
    name: "hvigor-arkts-compose",
    root: "hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose",
    includeDirs: ["dist"],
    includeFiles: ["index.js"],
  },
  {
    name: "hvigor-common",
    root: "hvigor-ohos-plugin/node_modules/@ohos/hvigor-common",
    includeDirs: ["src"],
    includeFiles: ["index.js"],
  },
  {
    name: "hvigor-logger",
    root: "hvigor-ohos-plugin/node_modules/@ohos/hvigor-logger",
    includeDirs: ["src"],
    includeFiles: ["index.js"],
  },
];

function walkFiles(dir, files = []) {
  if (!fs.existsSync(dir)) {
    return files;
  }
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules") {
        continue;
      }
      walkFiles(fullPath, files);
    } else if (entry.isFile() && (entry.name.endsWith(".ts") || entry.name.endsWith(".d.ts"))) {
      files.push(fullPath);
    } else if (entry.isFile() && entry.name.endsWith(".js")) {
      files.push(fullPath);
    }
  }
  return files;
}

function collectPackageFiles(spec) {
  const packageRoot = path.join(sourceRoot, spec.root);
  const files = [];
  for (const file of spec.includeFiles || []) {
    const fullPath = path.join(packageRoot, file);
    if (fs.existsSync(fullPath)) {
      files.push(fullPath);
      const declaration = fullPath.replace(/\.js$/, ".d.ts");
      if (fs.existsSync(declaration)) {
        files.push(declaration);
      }
    }
  }
  for (const dir of spec.includeDirs || []) {
    walkFiles(path.join(packageRoot, dir), files);
  }
  return [...new Set(files)].sort();
}

function isStringLiteral(node) {
  return t.isStringLiteral(node) || t.isLiteral?.(node) && typeof node.value === "string";
}

function stringValue(node) {
  return node.value;
}

function isRequireCall(node) {
  return t.isCallExpression(node) &&
    t.isIdentifier(node.callee, { name: "require" }) &&
    node.arguments.length === 1 &&
    isStringLiteral(node.arguments[0]);
}

function requireSource(node) {
  return stringValue(node.arguments[0]).replace(/\.js$/, "");
}

function isHelperImportCall(node, helperName) {
  return t.isCallExpression(node) &&
    t.isIdentifier(node.callee, { name: helperName }) &&
    node.arguments.length === 1 &&
    isRequireCall(node.arguments[0]);
}

function isExportsMember(node, name) {
  return t.isMemberExpression(node) &&
    t.isIdentifier(node.object, { name: "exports" }) &&
    t.isIdentifier(node.property, name ? { name } : undefined);
}

function exportNameOf(node) {
  if (!t.isMemberExpression(node) || !t.isIdentifier(node.object, { name: "exports" })) {
    return null;
  }
  if (t.isIdentifier(node.property)) {
    return node.property.name;
  }
  if (isStringLiteral(node.property)) {
    return stringValue(node.property);
  }
  return null;
}

function isEsModuleDefineProperty(node) {
  return t.isCallExpression(node) &&
    t.isMemberExpression(node.callee) &&
    t.isIdentifier(node.callee.object, { name: "Object" }) &&
    t.isIdentifier(node.callee.property, { name: "defineProperty" }) &&
    node.arguments.length >= 2 &&
    t.isIdentifier(node.arguments[0], { name: "exports" }) &&
    isStringLiteral(node.arguments[1]) &&
    stringValue(node.arguments[1]) === "__esModule";
}

function exportGetter(node) {
  if (!t.isCallExpression(node) ||
      !t.isMemberExpression(node.callee) ||
      !t.isIdentifier(node.callee.object, { name: "Object" }) ||
      !t.isIdentifier(node.callee.property, { name: "defineProperty" }) ||
      node.arguments.length < 3 ||
      !t.isIdentifier(node.arguments[0], { name: "exports" }) ||
      !isStringLiteral(node.arguments[1]) ||
      !t.isObjectExpression(node.arguments[2])) {
    return null;
  }
  const exported = stringValue(node.arguments[1]);
  if (exported === "__esModule") {
    return null;
  }
  const getter = node.arguments[2].properties.find((prop) =>
    t.isObjectProperty(prop) &&
    t.isIdentifier(prop.key, { name: "get" }) &&
    (t.isFunctionExpression(prop.value) || t.isArrowFunctionExpression(prop.value)));
  if (!getter) {
    return null;
  }
  const body = getter.value.body;
  if (!t.isBlockStatement(body) || body.body.length !== 1 || !t.isReturnStatement(body.body[0])) {
    return null;
  }
  const returned = body.body[0].argument;
  if (t.isIdentifier(returned)) {
    return { exported, local: returned.name };
  }
  if (t.isMemberExpression(returned) && t.isIdentifier(returned.property)) {
    return { exported, local: returned.property.name };
  }
  return null;
}

function isVoidExportAssignment(expression) {
  return t.isAssignmentExpression(expression) &&
    exportNameOf(expression.left) &&
    t.isUnaryExpression(expression.right, { operator: "void" });
}

function splitExpression(expression) {
  return t.isSequenceExpression(expression) ? expression.expressions : [expression];
}

function declarationHasOnlyHelpers(statement) {
  return t.isVariableDeclaration(statement) &&
    statement.declarations.length > 0 &&
    statement.declarations.every((decl) => t.isIdentifier(decl.id) && helperNames.has(decl.id.name));
}

function collectVoidExportNames(expression, names = []) {
  if (isVoidExportAssignment(expression)) {
    names.push(exportNameOf(expression.left));
    return names;
  }
  if (t.isAssignmentExpression(expression)) {
    const name = exportNameOf(expression.left);
    if (!name) {
      return null;
    }
    const rest = collectVoidExportNames(expression.right, names);
    if (!rest) {
      return null;
    }
    names.push(name);
    return names;
  }
  return null;
}

function makeImport(kind, local, source) {
  if (kind === "default") {
    return t.importDeclaration(
      [t.importDefaultSpecifier(t.identifier(local))],
      t.stringLiteral(source),
    );
  }
  return t.importDeclaration(
    [t.importNamespaceSpecifier(t.identifier(local))],
    t.stringLiteral(source),
  );
}

function markDeclarationExport(statement, exportNames) {
  if ((t.isClassDeclaration(statement) || t.isFunctionDeclaration(statement)) &&
      statement.id &&
      exportNames.has(statement.id.name)) {
    exportNames.delete(statement.id.name);
    return t.exportNamedDeclaration(statement, []);
  }
  if (t.isVariableDeclaration(statement)) {
    const exported = statement.declarations
      .filter((decl) => t.isIdentifier(decl.id) && exportNames.has(decl.id.name))
      .map((decl) => decl.id.name);
    if (exported.length === statement.declarations.length && exported.length > 0) {
      for (const name of exported) {
        exportNames.delete(name);
      }
      return t.exportNamedDeclaration(statement, []);
    }
  }
  return statement;
}

function splitGeneratedPreamble(source) {
  let text = source.replace(/^\/\*[\s\S]*?\*\/\s*/, "");
  let shebang = "";
  if (text.startsWith("#!")) {
    const newline = text.indexOf("\n");
    if (newline === -1) {
      shebang = text;
      text = "";
    } else {
      shebang = text.slice(0, newline);
      text = text.slice(newline + 1);
    }
  }
  return { shebang, text };
}

function readableHeader(sourceRel, shebang = "") {
  const lines = [];
  if (shebang) {
    lines.push(shebang);
  }
  lines.push(
    "/*",
    " * Human-readable TypeScript reconstruction.",
    " * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.",
    " * Keep semantic edits in Rust ports or separate hand-refined notes.",
    ` * Source baseline: ${sourceRel}`,
    " */",
    "",
  );
  return lines.join("\n");
}

function transformImplementation(source, sourceRel) {
  const { shebang, text } = splitGeneratedPreamble(source);
  const ast = parser.parse(text, {
    sourceType: "script",
    plugins: ["classProperties", "dynamicImport", "optionalChaining"],
    errorRecovery: true,
  });
  ast.program.directives = [];

  const defaultAliases = new Set();
  const importStatements = [];
  const exportNames = new Set();
  const exportAliases = [];
  let defaultExport = null;
  const body = [];

  for (const statement of ast.program.body) {
    if (t.isExpressionStatement(statement) && t.isStringLiteral(statement.expression, { value: "use strict" })) {
      continue;
    }
    if (declarationHasOnlyHelpers(statement)) {
      continue;
    }
    if (t.isVariableDeclaration(statement)) {
      const remaining = [];
      for (const decl of statement.declarations) {
        if (t.isIdentifier(decl.id) && helperNames.has(decl.id.name)) {
          continue;
        }
        if (!t.isIdentifier(decl.id) || !decl.init) {
          remaining.push(decl);
          continue;
        }
        if (isRequireCall(decl.init)) {
          importStatements.push(makeImport("namespace", decl.id.name, requireSource(decl.init)));
          continue;
        }
        if (isHelperImportCall(decl.init, "__importDefault")) {
          defaultAliases.add(decl.id.name);
          importStatements.push(makeImport("default", decl.id.name, requireSource(decl.init.arguments[0])));
          continue;
        }
        if (isHelperImportCall(decl.init, "__importStar")) {
          importStatements.push(makeImport("namespace", decl.id.name, requireSource(decl.init.arguments[0])));
          continue;
        }
        remaining.push(decl);
      }
      if (remaining.length > 0) {
        body.push(t.variableDeclaration(statement.kind, remaining));
      }
      continue;
    }
    if (t.isExpressionStatement(statement)) {
      const kept = [];
      for (const expression of splitExpression(statement.expression)) {
        if (isEsModuleDefineProperty(expression)) {
          continue;
        }
        const voidExportNames = collectVoidExportNames(expression);
        if (voidExportNames) {
          for (const name of voidExportNames) {
            exportNames.add(name);
          }
          continue;
        }
        const getter = exportGetter(expression);
        if (getter) {
          exportAliases.push(getter);
          continue;
        }
        if (t.isCallExpression(expression) &&
            t.isIdentifier(expression.callee, { name: "__exportStar" }) &&
            expression.arguments.length >= 2 &&
            isRequireCall(expression.arguments[0])) {
          body.push(t.exportAllDeclaration(t.stringLiteral(requireSource(expression.arguments[0]))));
          continue;
        }
        if (t.isAssignmentExpression(expression)) {
          const name = exportNameOf(expression.left);
          if (name) {
            if (name === "default") {
              defaultExport = expression.right;
              continue;
            }
            if (t.isIdentifier(expression.right, { name })) {
              exportNames.add(name);
              continue;
            }
            if (t.isIdentifier(expression.right)) {
              exportAliases.push({ exported: name, local: expression.right.name });
              continue;
            }
            body.push(t.exportNamedDeclaration(
              t.variableDeclaration("const", [t.variableDeclarator(t.identifier(name), expression.right)]),
              [],
            ));
            continue;
          }
        }
        kept.push(expression);
      }
      if (kept.length === 1) {
        body.push(t.expressionStatement(kept[0]));
      } else if (kept.length > 1) {
        body.push(t.expressionStatement(t.sequenceExpression(kept)));
      }
      continue;
    }
    body.push(statement);
  }

  ast.program.body = [...importStatements, ...body.map((statement) => markDeclarationExport(statement, exportNames))];

  const alreadyExported = new Set();
  for (const statement of ast.program.body) {
    if (t.isExportNamedDeclaration(statement) && statement.declaration) {
      const decl = statement.declaration;
      if ((t.isClassDeclaration(decl) || t.isFunctionDeclaration(decl)) && decl.id) {
        alreadyExported.add(decl.id.name);
      }
      if (t.isVariableDeclaration(decl)) {
        for (const variable of decl.declarations) {
          if (t.isIdentifier(variable.id)) {
            alreadyExported.add(variable.id.name);
          }
        }
      }
    }
  }

  const finalSpecs = [];
  for (const name of [...exportNames].sort()) {
    if (!alreadyExported.has(name)) {
      finalSpecs.push(t.exportSpecifier(t.identifier(name), t.identifier(name)));
    }
  }
  for (const { exported, local } of exportAliases) {
    finalSpecs.push(t.exportSpecifier(t.identifier(local), t.identifier(exported)));
  }
  if (finalSpecs.length > 0) {
    ast.program.body.push(t.exportNamedDeclaration(null, finalSpecs));
  }
  if (defaultExport) {
    ast.program.body.push(t.exportDefaultDeclaration(defaultExport));
  }

  traverse(ast, {
    noScope: true,
    MemberExpression(path) {
      const node = path.node;
      if (t.isIdentifier(node.property, { name: "default" }) &&
          t.isIdentifier(node.object) &&
          defaultAliases.has(node.object.name)) {
        path.replaceWith(t.identifier(node.object.name));
        return;
      }
      const name = exportNameOf(node);
      if (name) {
        path.replaceWith(t.identifier(name));
      }
    },
    UnaryExpression(path) {
      const node = path.node;
      if (node.operator === "!" && t.isNumericLiteral(node.argument)) {
        if (node.argument.value === 0) {
          path.replaceWith(t.booleanLiteral(true));
        } else if (node.argument.value === 1) {
          path.replaceWith(t.booleanLiteral(false));
        }
      } else if (node.operator === "void" && t.isNumericLiteral(node.argument, { value: 0 })) {
        path.replaceWith(t.identifier("undefined"));
      }
    },
  });

  const code = generate(ast, {
    comments: false,
    retainLines: false,
    compact: false,
    jsescOption: { minimal: true },
  }).code;

  return readableHeader(sourceRel, shebang) + code + "\n";
}

function copyDeclaration(source, sourceRel) {
  const { shebang, text } = splitGeneratedPreamble(source);
  return readableHeader(sourceRel, shebang) + text.replace(/\.js(["'])/g, "$1");
}

function main() {
  if (!fs.existsSync(sourceRoot)) {
    throw new Error(`missing source directory: ${sourceRoot}`);
  }
  fs.rmSync(outputRoot, { recursive: true, force: true });
  fs.mkdirSync(outputRoot, { recursive: true });

  const records = [];
  for (const spec of packageSpecs) {
    const packageRoot = path.join(sourceRoot, spec.root);
    for (const file of collectPackageFiles(spec)) {
      const sourceRel = path.relative(packageRoot, file);
      const outRel = file.endsWith(".js") ? sourceRel.replace(/\.js$/, ".ts") : sourceRel;
      const source = fs.readFileSync(file, "utf8");
      const outFile = path.join(outputRoot, spec.name, outRel);
      fs.mkdirSync(path.dirname(outFile), { recursive: true });
      let output;
      try {
        output = file.endsWith(".d.ts")
          ? copyDeclaration(source, `${spec.name}/${sourceRel}`)
          : transformImplementation(source, `${spec.name}/${sourceRel}`);
      } catch (error) {
        error.message = `${spec.name}/${sourceRel}: ${error.message}`;
        throw error;
      }
      fs.writeFileSync(outFile, output);
      records.push({
        source: path.relative(sourceRoot, file),
        output: path.relative(repoRoot, outFile),
        kind: file.endsWith(".d.ts") ? "declaration" : "implementation",
      });
    }
  }

  const manifest = {
    generatedAt: new Date().toISOString(),
    sourceRoot,
    outputRoot: path.relative(repoRoot, outputRoot),
    implementationCount: records.filter((record) => record.kind === "implementation").length,
    declarationCount: records.filter((record) => record.kind === "declaration").length,
    transforms: [
      "remove CommonJS module markers and TypeScript helper declarations",
      "convert top-level require declarations to ESM import declarations",
      "convert exports assignments and getter re-exports to ESM exports",
      "replace helper default alias .default access with direct default identifiers",
      "normalize !0/!1 and void 0 into true/false/undefined",
    ],
    files: records,
  };
  fs.writeFileSync(
    path.join(repoRoot, "hvigor", "manifests", "ts-readable-reconstruction.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );

  fs.writeFileSync(
    path.join(outputRoot, "README.md"),
    [
      "# Human-Readable hvigor TypeScript Reconstruction",
      "",
      "This directory is generated directly from the local DevEco Studio hvigor JavaScript distribution with CommonJS compiler glue removed.",
      "",
      "It is intended for behavior analysis and Rust architecture planning. It is not a runnable replacement for hvigor.",
      "",
      "Regenerate:",
      "",
      "```bash",
      "node hvigor/tools/reconstruct-readable-ts.cjs",
      "```",
      "",
    ].join("\n"),
  );

  console.log(JSON.stringify({
    outputRoot: path.relative(repoRoot, outputRoot),
    implementationCount: manifest.implementationCount,
    declarationCount: manifest.declarationCount,
  }, null, 2));
}

main();
