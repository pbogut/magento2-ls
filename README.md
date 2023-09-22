# Magento 2 Language Server

## Overview

The Magento 2 Language Server is a tool that acts as a bridge between Magento 2 XML and PHP files. It provides features such as jumping to the definition of PHP classes from XML definitions of plugins, observers, jobs, and similar entities. The goal is to also provide code completion suggestions in XML files when referencing PHP classes and functions, and potentially finding references of PHP classes/methods in XML in the future.

Please note that the current version of the language server is considered to be of alpha quality. While it works and can be used, it has limited functionality and things can break.

## Features
 - Jump to definition from XML files:
   - Jump to class (from `<plugin/>`, `<observer/>`, `<argument xsi:type="object"/>`, etc.)
   - Jump to constant (from `<argument xsi:type="init_parameter"/>`)
   - Jump to method (from `<service/>`, `<job/>`)
   - Jump to template file (from `<block/>`, `<referenceBlock/>`, etc.)

### Planned (not implemented yet)
 - Code completion suggestions in XML files when referencing PHP classes and functions.
 - Finding of references of PHP classes/methods in XML files.

## Installation

### Neovim (with Packer)

Add the following lines to your init.lua file if you are using Packer as your plugin manager:

```lua
use({ 'pbogut/magento2-ls', 
  run = 'cargo build --release',
  config = "require'magento2_ls'.setup()" 
})
```

The `require('magento2_ls').setup()` command will register the language server with Neovim's built-in Language Server Protocol (LSP) using the `vim.lsp.start()` function. If you need to rebuild the language server for any reason, you can do so with:

```lua
require('magento2_ls').build()
```

### Visual Studio Code

Support for Visual Studio Code is planned and will be added in a future update.


### Non goals

Be PHP Language Server (or XML LS) in any capacity. 
[Intelephense](https://intelephense.com/) works nice with Magento 2 if you need 
LS for your project.

## Contributing

If you would like to contribute, please feel free to submit pull requests or open issues on the [GitHub repository](https://github.com/pbogut/magento2-ls). 

## License

The Magento 2 Language Server is released under the MIT License.
The software is provided "as is", without warranty of any kind.
