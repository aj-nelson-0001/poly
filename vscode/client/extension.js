"use strict";

const fs = require("fs");
const vscode = require("vscode");
const {
  LanguageClient,
  TransportKind,
} = require("vscode-languageclient/node");

let client = null;

/**
 * Resolve the poly-lsp server command from settings, falling back to the
 * `poly-lsp` binary on PATH.
 */
function resolveServerPath() {
  const configured = vscode.workspace
    .getConfiguration("poly")
    .get("lsp.path", "poly-lsp");
  if (configured && configured.trim() !== "") {
    return configured.trim();
  }
  return "poly-lsp";
}

function startLanguageClient() {
  const serverOptions = {
    command: resolveServerPath(),
    transport: TransportKind.stdio,
  };

  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "poly" }],
    synchronize: {
      configurationSection: "poly",
    },
    traceOutputChannel: vscode.window.createOutputChannel("Poly LSP"),
    initializationOptions: {},
  };

  client = new LanguageClient(
    "poly-lsp",
    "Poly Language Server",
    serverOptions,
    clientOptions
  );

  client.onDidChangeState((event) => {
    if (event.newState.name === "Running") {
      vscode.window.setStatusBarMessage(
        "$(pulse) Poly language server running",
        3000
      );
    }
  });

  return client.start();
}

async function stopLanguageClient() {
  if (client) {
    await client.stop();
    client = null;
  }
}

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("poly.restartLanguageServer", async () => {
      await stopLanguageClient();
      await startLanguageClient();
      vscode.window.showInformationMessage("Poly language server restarted");
    })
  );

  const serverPath = resolveServerPath();
  // A filesystem path (contains a slash) can be checked directly; a bare
  // command name is assumed to be on PATH and checked by the client start.
  if (serverPath.includes("/") && !fs.existsSync(serverPath)) {
    vscode.window.showWarningMessage(
      `Poly: language server binary not found at "${serverPath}". Set the 'poly.lsp.path' setting to the correct location.`
    );
  }

  startLanguageClient().catch((error) => {
    vscode.window.showErrorMessage(
      `Poly: failed to start language server: ${error.message ?? error}`
    );
  });
}

function deactivate() {
  return stopLanguageClient();
}

module.exports = { activate, deactivate };
