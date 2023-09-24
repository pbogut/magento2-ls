// @ts-check
const { LanguageClient } = require("vscode-languageclient/node");
const tmpdir = require("os").tmpdir();

module.exports = {
  /** @param {import("vscode").ExtensionContext} context*/
  activate(context) {
    const command =
      context.asAbsolutePath("server/magento2-ls") +
      (process.platform === "win32" ? ".exe" : "");
    /** @type {import("vscode-languageclient/node").ServerOptions} */
    const serverOptions = {
      run: {
        command: command,
      },
      debug: {
        command: command,
      },
    };

    /** @type {import("vscode-languageclient/node").LanguageClientOptions} */
    const clientOptions = {
      documentSelector: [{ scheme: "file", language: "xml" }],
    };

    const client = new LanguageClient(
      "magento2-ls",
      "Magento 2 Language Server",
      serverOptions,
      clientOptions,
    );

    client.start();
  },
};

