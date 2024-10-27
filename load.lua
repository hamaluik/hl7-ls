vim.api.nvim_create_autocmd('FileType', {
  pattern = 'hl7',
  callback = function(args)
    vim.lsp.start({
      name = 'hl7-ls',
      cmd = {'/home/kenton/Documents/projects/hl7-ls/target/debug/hl7-ls'},
      root_dir = vim.fs.root(args.buf, {'*.hl7'}),
      cmd_env = {
        RUST_LOG = 'hl7_ls=trace',
      },
      offset_encoding = 'utf-8',
    })
  end,
})
