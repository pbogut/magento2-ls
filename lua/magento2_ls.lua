local M = {}

local function script_path(append)
  append = append or ''
  local str = debug.getinfo(1, 'S').source:sub(2)
  str = str:match('(.*[/\\])') or './'
  return str .. append
end

local clientId = nil

local destination = script_path('../target/release')

local function start_lsp(opts)
  if clientId ~= nil then
    vim.lsp.buf_attach_client(0, clientId)
  else
    clientId = vim.lsp.start(opts)
  end
end

---@return string|nil
local function get_machine()
  local machine = vim.loop.os_uname().machine
  if machine == 'x86_64' then
    return 'x64'
  elseif machine == 'aarch64' or machine == 'arm64' then
    return 'arm64'
  end
end

---@return string|nil
local function get_system()
  local os = vim.loop.os_uname().sysname
  if os == 'Linux' then
    return 'linux'
  elseif os == 'Darwin' then
    return 'darwin'
  elseif os == 'Windows' then
    return 'windows'
  end
end

---@return string|nil
local function get_package()
  local system = get_system()
  local machine = get_machine()
  if system and machine then
    return system .. '-' .. machine
  end
end

---@return string
local function get_version()
  local file = io.open(script_path('../Cargo.toml'), 'r')
  if file ~= nil then
    for line in file:lines() do
      if line:match('^version = "(.*)"$') then
        local version = line:match('^version = "(.*)"$')
        return version
      end
    end
  end

  return '0.0.0'
end

---@return string|nil
local function get_bin_name()
  local package = get_package()
  if not package then
    return nil
  end
  if get_system() == 'windows' then
    return 'magento2-ls-' .. package .. '.exe'
  else
    return 'magento2-ls-' .. package
  end
end

---@param bin_name string
---@return string
local function get_bin_url(bin_name)
  local base_url = 'https://github.com/pbogut/magento2-ls/releases/download/' .. get_version()
  return base_url .. '/' .. bin_name
end

---@param bin_url string
local function download_server(bin_url)
  local bin = destination .. '/magento2-ls'
  if get_system() == 'windows' then
    bin = bin .. '.exe'
  end
  vim.fn.mkdir(destination, 'p')
  local cmd = 'curl -L -o ' .. bin .. ' ' .. bin_url
  vim.fn.jobstart(cmd, {
    on_exit = function(_, code)
      if code == 0 then
        vim.notify('Server download successful', vim.log.levels.INFO, { title = 'magento2-ls' })
        if get_system() ~= 'windows' then
          vim.fn.system('chmod +x ' .. bin)
        end
      else
        vim.notify('Server download failed', vim.log.levels.ERROR, { title = 'magento2-ls' })
      end
    end,
  })
end

M.setup = function(opts)
  opts = opts or {}
  opts = vim.tbl_deep_extend('keep', opts, {
    filetypes = { 'xml', 'javascript' },
    name = 'magento2-ls',
    cmd = { script_path('../target/release/magento2-ls') .. (get_system() == 'windows' and '.exe' or '') },
    root_dir = vim.fn.getcwd(),
  })

  for _, ft in ipairs(opts.filetypes) do
    if ft == vim.o.filetype then
      start_lsp(opts)
    end
  end

  local augroup = vim.api.nvim_create_augroup('magento2_ls', { clear = true })
  local pattern = table.concat(opts.filetypes, ',')

  vim.api.nvim_create_autocmd('FileType', {
    group = augroup,
    pattern = pattern,
    callback = function()
      start_lsp(opts)
    end,
  })
end

M.get_server = function()
  local bin_name = get_bin_name()
  local bin_url = ''
  if bin_name then
    bin_url = get_bin_url(bin_name)
    download_server(bin_url)
  else
    vim.ui.select({
      'magento2-ls-darwin-arm64',
      'magento2-ls-darwin-x64',
      'magento2-ls-linux-x64',
      'magento2-ls-windows-arm64.exe',
      'magento2-ls-windows-x64.exe',
    }, {
      prompt = 'Select package for your system',
    }, function(selected)
      if selected then
        bin_url = get_bin_url(selected)
        download_server(bin_url)
      end
    end)
  end
end

M.build = function()
  local cmd = 'cd ' .. vim.fn.shellescape(script_path('..')) .. ' && cargo build --release'
  vim.fn.jobstart(cmd, {
    on_exit = function(_, code)
      if code == 0 then
        vim.notify('Build successful', vim.log.levels.INFO, { title = 'magento2-ls' })
      else
        vim.notify('Build failed', vim.log.levels.ERROR, { title = 'magento2-ls' })
      end
    end,
  })
end

return M
