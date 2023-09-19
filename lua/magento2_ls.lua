local M = {}

local function script_path(append)
  append = append or ''
  local str = debug.getinfo(1, 'S').source:sub(2)
  str = str:match('(.*[/\\])') or './'
  return str .. append
end

local clientId = nil

local function start_lsp(opts)
  if clientId ~= nil then
    vim.lsp.buf_attach_client(0, clientId)
  else
    clientId = vim.lsp.start(opts)
  end
end

M.setup = function(opts)
  opts = opts or {}
  opts = vim.tbl_deep_extend('keep', opts, {
    filetypes = { 'xml' },
    name = 'magento2-ls',
    cmd = { script_path('../target/release/magento2-ls') },
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
