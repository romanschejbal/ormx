import { workspace, ExtensionContext, window } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import { execSync } from "child_process";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext): void {
  const serverPath = resolveServerPath();
  if (!serverPath) {
    window.showWarningMessage(
      "ferriorm-lsp not found. Install it with `cargo install ferriorm-lsp` " +
        "or set `ferriorm.lsp.serverPath` to the binary path."
    );
    return;
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "ferriorm" },
      { scheme: "untitled", language: "ferriorm" },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.ferriorm"),
    },
  };

  client = new LanguageClient(
    "ferriorm",
    "ferriorm Language Server",
    serverOptions,
    clientOptions
  );
  client.start();
  context.subscriptions.push({ dispose: () => client?.stop() });
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

function resolveServerPath(): string | undefined {
  const configured = workspace
    .getConfiguration("ferriorm")
    .get<string>("lsp.serverPath");
  if (configured && configured.length > 0) {
    return configured;
  }
  // Look for `ferriorm-lsp` on PATH using `which` / `where`.
  const cmd = process.platform === "win32" ? "where ferriorm-lsp" : "which ferriorm-lsp";
  try {
    const out = execSync(cmd, { stdio: ["ignore", "pipe", "ignore"] })
      .toString()
      .trim()
      .split(/\r?\n/)[0];
    return out || undefined;
  } catch {
    return undefined;
  }
}
