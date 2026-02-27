const vscode = require('vscode');

class NullbyteDebugConfigProvider {
  resolveDebugConfiguration(_folder, config) {
    if (!config.type) {
      config.type = 'nullbyte-directive';
    }
    if (!config.request) {
      config.request = 'launch';
    }
    if (!config.name) {
      config.name = 'Launch Nullbyte Directive';
    }
    if (!config.program && vscode.window.activeTextEditor) {
      config.program = vscode.window.activeTextEditor.document.fileName;
    }
    if (!config.cwd && vscode.workspace.workspaceFolders?.[0]) {
      config.cwd = vscode.workspace.workspaceFolders[0].uri.fsPath;
    }
    if (config.hotReloadOnSave === undefined) {
      config.hotReloadOnSave = true;
    }
    return config;
  }
}

class NullbyteDebugAdapterFactory {
  createDebugAdapterDescriptor(session) {
    const workspaceCwd =
      session?.configuration?.cwd ||
      vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ||
      process.cwd();

    return new vscode.DebugAdapterExecutable(
      'cargo',
      ['run', '--quiet', '-p', 'emulator-debug-session', '--bin', 'nullbyte-dap'],
      { cwd: workspaceCwd },
    );
  }
}

function activate(context) {
  const configProvider = new NullbyteDebugConfigProvider();
  const factory = new NullbyteDebugAdapterFactory();

  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider(
      'nullbyte-directive',
      configProvider,
    ),
  );

  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(
      'nullbyte-directive',
      factory,
    ),
  );

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(async (document) => {
      const session = vscode.debug.activeDebugSession;
      if (!session || session.type !== 'nullbyte-directive') {
        return;
      }
      if (session.configuration?.hotReloadOnSave === false) {
        return;
      }

      const fileName = String(document.fileName || '').toLowerCase();
      if (!(fileName.endsWith('.n1') || fileName.endsWith('.n1.md'))) {
        return;
      }

      try {
        await session.customRequest('restart', {
          arguments: {
            mode: 'hot_patch',
            source: document.fileName,
            trigger: 'save',
          },
        });
      } catch (error) {
        const message = String(error?.message || error);
        vscode.window.setStatusBarMessage(
          `Nullbyte reload failed: ${message}`,
          4000,
        );
      }
    }),
  );
}

function deactivate() {}

module.exports = {
  activate,
  deactivate,
};
