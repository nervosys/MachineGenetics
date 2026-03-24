import * as vscode from "vscode";
import { RapClient } from "./rap-client";

let rapClient: RapClient | undefined;

export function activate(context: vscode.ExtensionContext) {
  console.log("MechGen extension activated");

  // Register RAP start/stop commands.
  context.subscriptions.push(
    vscode.commands.registerCommand("MechGen.startRap", () => {
      const config = vscode.workspace.getConfiguration("MechGen");
      const addr = config.get<string>("rapAddress", "127.0.0.1:9876");
      rapClient = new RapClient(addr);
      rapClient
        .connect()
        .then(() => vscode.window.showInformationMessage(`RAP connected: ${addr}`))
        .catch((err: Error) =>
          vscode.window.showErrorMessage(`RAP connection failed: ${err.message}`)
        );
    }),

    vscode.commands.registerCommand("MechGen.stopRap", () => {
      if (rapClient) {
        rapClient.disconnect();
        rapClient = undefined;
        vscode.window.showInformationMessage("RAP disconnected");
      }
    }),

    // One-click Convert to MechGen from .rs context menu.
    vscode.commands.registerCommand("MechGen.convertToRedox", async (uri: vscode.Uri) => {
      if (!uri) {
        vscode.window.showWarningMessage("No file selected");
        return;
      }
      vscode.window.showInformationMessage(
        `Migration: run \`rust2mg ${uri.fsPath}\` to convert.`
      );
    })
  );

  // Register diagnostics provider via RAP.
  const diagnosticCollection = vscode.languages.createDiagnosticCollection("MechGen");
  context.subscriptions.push(diagnosticCollection);

  // Trigger diagnostics on save for .mg files.
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(async (doc) => {
      if (doc.languageId !== "MechGen" || !rapClient) {
        return;
      }

      try {
        const result = await rapClient.request("build/check", {
          source: doc.getText(),
        });

        const diagnostics: vscode.Diagnostic[] = [];
        const errors = result?.errors;
        if (Array.isArray(errors)) {
          for (const err of errors) {
            const line = (err.line ?? 1) - 1;
            const col = err.col ?? 0;
            const range = new vscode.Range(line, col, line, col + (err.len ?? 1));
            const diag = new vscode.Diagnostic(
              range,
              err.message ?? "unknown error",
              vscode.DiagnosticSeverity.Error
            );
            diagnostics.push(diag);
          }
        }

        diagnosticCollection.set(doc.uri, diagnostics);
      } catch {
        // RAP not available — silently skip.
      }
    })
  );

  // Hover provider — queries RAP for type info.
  context.subscriptions.push(
    vscode.languages.registerHoverProvider("MechGen", {
      async provideHover(doc, position) {
        if (!rapClient) return undefined;

        try {
          const result = await rapClient.request("language/hover", {
            source: doc.getText(),
            line: position.line + 1,
            col: position.character,
          });

          if (result?.info) {
            return new vscode.Hover(
              new vscode.MarkdownString(`**${result.info}**\n\n${result.doc ?? ""}`)
            );
          }
        } catch {
          // RAP not available.
        }

        return undefined;
      },
    })
  );
}

export function deactivate() {
  if (rapClient) {
    rapClient.disconnect();
    rapClient = undefined;
  }
}
