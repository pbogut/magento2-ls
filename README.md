# Magento 2 Language Server

## Overview

The Magento 2 Language Server is a tool that serves as a connection between Magento 2 XML, JS and PHP files, with the goal of enabling easier navigation between these files.

Please note that the current version of the language server is considered to be of alpha quality. Although it is functional and can be used, it has limited functionality and may encounter issues. It has been tested on Linux and should also work on MacOS and Windows.

## Features
- Go to the definition from XML files:
   - Go to the class (from `<plugin/>`, `<observer/>`, `<argument xsi:type="object"/>`, etc.)
   - Go to the constant (from `<argument xsi:type="init_parameter"/>`)
   - Go to the method (from `<service/>`, `<job/>`)
   - Go to the template file (from `<block/>`, `<referenceBlock/>`, etc.)
   - Go to the JavaScript component file (from `<item name="component" xsi:type="string"/>`)
 - Go to the definition from JS files:
   - Go to the JavaScript component file (from `define()` argument list)

### Planned (not implemented yet)
 - Code completion suggestions in XML files when referencing PHP classes and functions.
 - Finding of references of PHP classes/methods in XML files.

## Installation

### Neovim (with Packer)

Please add the following lines to your init.lua file if you are using Packer as your plugin manager.

```lua
use({ 'pbogut/magento2-ls', 
  -- Build using cargo build --release
  run = "require'magento2_ls'.build()" ,
  -- Alternatively, you can download the compiled binary from the GitHub release.
  -- run = "require'magento2_ls'.get_server()" ,
  config = "require'magento2_ls'.setup()" 
})
```

The command `require('magento2_ls').setup()` will register the language server with Neovim's built-in Language Server Protocol (LSP) using the function `vim.lsp.start()`. If you need to rebuild the language server for any reason, you can do it by using:

```lua
require('magento2_ls').build()
```

Alternatively, you can download the compiled binary for your system by using:

```lua
require('magento2_ls').get_server()
```

### Visual Studio Code

You can download the `vsix` file from the [GitHub Releases](https://github.com/pbogut/magento2-ls/releases) page.

### Non goals

Be PHP Language Server (or XML LS) in any capacity. 
[Intelephense](https://intelephense.com/) works nice with Magento 2 if you need 
LS for your project.

## Contributing

If you would like to contribute, please feel free to submit pull requests or open issues on the [GitHub repository](https://github.com/pbogut/magento2-ls). 

## License

The Magento 2 Language Server is released under the MIT License.
The software is provided "as is", without warranty of any kind.
