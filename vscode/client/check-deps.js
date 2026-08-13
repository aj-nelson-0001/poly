"use strict";

// Verifies the language client dependency is present before packaging.
try {
  require.resolve("vscode-languageclient");
  console.log("check-deps: vscode-languageclient present");
} catch (error) {
  console.error(
    "check-deps: vscode-languageclient is missing. Run `npm install` in the vscode/ directory first."
  );
  process.exit(1);
}
