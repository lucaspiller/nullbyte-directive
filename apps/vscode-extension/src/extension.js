const path = require("node:path");
const vscode = require("vscode");

class NullbyteDebugConfigProvider {
  resolveDebugConfiguration(_folder, config) {
    if (!config.type) {
      config.type = "nullbyte-directive";
    }
    if (!config.request) {
      config.request = "launch";
    }
    if (!config.name) {
      config.name = "Launch Nullbyte Directive";
    }
    if (!config.program && vscode.window.activeTextEditor) {
      config.program = vscode.window.activeTextEditor.document.fileName;
    }
    if (!config.cwd && vscode.workspace.workspaceFolders?.[0]) {
      config.cwd = vscode.workspace.workspaceFolders[0].uri.fsPath;
    }
    return config;
  }
}

class NullbyteDebugAdapterFactory {
  constructor(context) {
    this.context = context;
  }

  createDebugAdapterDescriptor() {
    const adapterPath = this.context.asAbsolutePath(
      path.join("src", "dap", "adapter.js"),
    );
    return new vscode.DebugAdapterExecutable("node", [adapterPath]);
  }
}

function activate(context) {
  const configProvider = new NullbyteDebugConfigProvider();
  const factory = new NullbyteDebugAdapterFactory(context);

  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider(
      "nullbyte-directive",
      configProvider,
    ),
  );

  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(
      "nullbyte-directive",
      factory,
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "nullbyteDirective.startDebugging",
      async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
          return;
        }
        await vscode.debug.startDebugging(undefined, {
          type: "nullbyte-directive",
          request: "launch",
          name: "Launch Nullbyte Directive",
          program: editor.document.fileName,
          cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        });
      },
    ),
  );
}

function deactivate() {}

module.exports = {
  activate,
  deactivate,
};
