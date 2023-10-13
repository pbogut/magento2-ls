# Magento 2 Language Server

## Overview

The Magento 2 Language Server is a tool that serves as a connection between Magento 2 XML, JS and PHP files, with the goal of enabling easier navigation between these files.

Please note that the current version of the language server is considered to be of alpha quality. Although it is functional and can be used, it has limited functionality and may encounter issues. It has been tested on Linux and should also work on MacOS and Windows.

## Features

![go-to-definition](https://github.com/pbogut/magento2-ls/assets/1702152/20f556a8-5024-4a1b-befd-26ef1ded6000)

- Go to the definition from XML files:
   - Go to the class (from `<plugin/>`, `<observer/>`, `<argument xsi:type="object"/>`, etc.)
   - Go to the constant (from `<argument xsi:type="init_parameter"/>`)
   - Go to the method (from `<service/>`, `<job/>`)
   - Go to the template file (from `<block/>`, `<referenceBlock/>`, etc.)
   - Go to the JavaScript component file (from `<item name="component" xsi:type="string"/>`)
 - Go to the definition from JS files:
   - Go to the JavaScript component file (from `define()` argument list)

![code-completion](https://github.com/pbogut/magento2-ls/assets/1702152/6341cf9e-2241-40c2-b374-e45d7026e1bc)

- Completion of various Magento entities:
  - Template suggestions inside `template=""` attributes.
  - Template suggestions inside tags with `xsi:type="string"` and `name=template` attributes.
  - Event names inside `<event name="">` attribute (static list of built-in events).
  - PHP Class suggestions in `<preference for="">`, `<preference type="">`, `class`, and `instance` attributes.
  - PHP Class suggestions in tags with `xsi:type="object"` attribute.
  - PHP Class suggestions in `<backend_model/>`, `<frontend_model/>`, and `<source_model/>` tags.
  - JS Component suggestions in tags with `xsi:type="string"` and `name="component"` attributes.
  - JS Component suggestions in the argument list of the `define()` function in JavaScript files.

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

## Requirements

In order to complete Magento classes, the Magento root folder must be opened in the workspace.

The language server detects Magento modules by searching for `registration.php` files in the following locations:

- The root folder (for modules added to the workspace)
- `app/code/*/*/` - for local modules
- `vendor/*/*/` - for vendor modules
- `app/design/*/*/*/` - for themes.


## Contributing

If you would like to contribute, please feel free to submit pull requests or open issues on the [GitHub repository](https://github.com/pbogut/magento2-ls). 

## License

The Magento 2 Language Server is released under the MIT License.
The software is provided "as is", without warranty of any kind.
