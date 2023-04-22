local M = {}

local function script_path(append)
  append = append or ''
  local str = debug.getinfo(1, 'S').source:sub(2)
  str = str:match('(.*[/\\])') or './'
  return str .. append
end

local clientId = nil

M.setup = function(opts)
  opts = opts or {}
  opts = vim.tbl_deep_extend('keep', opts, {
    filetypes = { 'xml' },
    name = 'magento2-ls',
    cmd = { 'node', script_path('../out/server.js'), '--stdio' },
    root_dir = vim.fn.getcwd(),
  })

  local augroup = vim.api.nvim_create_augroup('magento2_ls', { clear = true })
  local pattern = table.concat(opts.filetypes, ',')

  vim.api.nvim_create_autocmd('FileType', {
    group = augroup,
    pattern = pattern,
    callback = function()
      if clientId ~= nil then
        vim.lsp.buf_attach_client(0, clientId)
      else
        clientId = vim.lsp.start(opts)
      end
    end,
  })
end

M.build = function()
  local cmd = 'cd ' .. vim.fn.shellescape(script_path('..')) .. ' && npm install && npm run build'
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
