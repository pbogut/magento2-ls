// @ts-check
const { workspace } = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

module.exports = {
  /** @param {import("vscode").ExtensionContext} context*/
  activate(context) {
    const extension = process.platform === "win32" ? ".exe" : "";

    /** @type {import("vscode-languageclient/node").ServerOptions} */
    const serverOptions = {
      run: {
        command: context.asAbsolutePath("server/magento2-ls") + extension,
      },
      debug: {
        command:
          context.asAbsolutePath("../target/debug/magento2-ls") + extension,
      },
    };

    /** @type {import("vscode-languageclient/node").LanguageClientOptions} */
    const clientOptions = {
      documentSelector: [
        { scheme: "file", language: "xml" },
        { scheme: "file", language: "javascript" },
      ],
    };

    const client = new LanguageClient(
      "magento2-ls",
      "Magento 2 Language Server",
      serverOptions,
      clientOptions,
    );

    workspace.onDidChangeWorkspaceFolders((_event) => {
      // TODO implement `workspace/didChangeWorkspaceFolders` in the server.
      // For now just restart server when workspace folders change
      client.restart();
    });

    client.start();
  },
};
