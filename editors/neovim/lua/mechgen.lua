-- MechGen language support for Neovim.
--
-- Installation:
--   1. Copy this file to ~/.config/nvim/lua/MechGen.lua
--   2. Add `require('MechGen')` to your init.lua
--
-- Or with lazy.nvim, add this directory as a local plugin.

local M = {}

-- ── Filetype detection ──────────────────────────────────────────────

vim.filetype.add({
  extension = {
    mg = 'MechGen',
  },
  filename = {
    ['Forge.toml'] = 'toml',
  },
})

-- ── LSP (RAP) ───────────────────────────────────────────────────────

function M.setup_lsp(opts)
  opts = opts or {}

  local lspconfig_ok, lspconfig = pcall(require, 'lspconfig')
  if not lspconfig_ok then
    vim.notify('MechGen: nvim-lspconfig not found', vim.log.levels.WARN)
    return
  end

  local configs = require('lspconfig.configs')

  -- Register RAP as a custom LSP server if not already defined.
  if not configs.rap then
    configs.rap = {
      default_config = {
        cmd = { opts.rap_cmd or 'rap' },
        filetypes = { 'MechGen' },
        root_dir = lspconfig.util.root_pattern('Forge.toml', '.git'),
        settings = {
          rap = {
            checkOnSave = true,
            diagnostics = {
              enable = true,
              skb = true,
              effects = true,
            },
            completion = {
              autoimport = true,
              sigils = true,
            },
            inlayHints = {
              enable = true,
              typeHints = true,
              effectHints = true,
              costHints = false,
            },
          },
        },
      },
    }
  end

  lspconfig.rap.setup(opts.lsp or {})
end

-- ── Treesitter ──────────────────────────────────────────────────────

function M.setup_treesitter()
  local ts_ok, parsers = pcall(require, 'nvim-treesitter.parsers')
  if not ts_ok then
    return
  end

  local parser_config = parsers.get_parser_configs()
  parser_config.MechGen = {
    install_info = {
      -- When a tree-sitter-MechGen parser is published, point url here.
      url = 'https://github.com/nervosys/tree-sitter-MechGen',
      files = { 'src/parser.c' },
      branch = 'main',
    },
    filetype = 'MechGen',
  }
end

-- ── Syntax (fallback vim regex, used when tree-sitter is unavailable) ──

function M.setup_syntax()
  vim.api.nvim_create_autocmd('FileType', {
    pattern = 'MechGen',
    callback = function()
      local buf = vim.api.nvim_get_current_buf()

      -- Comments.
      vim.bo[buf].commentstring = '// %s'

      -- Basic syntax groups via vim.cmd (fallback if no tree-sitter).
      vim.cmd([[
        if exists('b:current_syntax') | finish | endif

        " Comments
        syn match   mechgenComment      "//.*$"
        syn region  mechgenCommentBlock start="/\*" end="\*/"

        " Strings
        syn region  mechgenString       start='"' end='"' contains=mechgenEscape,mechgenInterp
        syn region  mechgenPrintStr     start='p"' end='"' contains=mechgenEscape,mechgenInterp
        syn region  mechgenFmtStr       start='f"' end='"' contains=mechgenEscape,mechgenInterp
        syn match   mechgenEscape       contained "\\."
        syn region  mechgenInterp       contained start="{" end="}"

        " Declarations
        syn match   mechgenFnDecl       "\v(\+f|~f|\baf\b|\bf\b)\s+[a-zA-Z_]\w*"
        syn match   mechgenStructDecl   "\v(\+S|\bS\b)\s+[A-Z]\w*"
        syn match   mechgenEnumDecl     "\v(\+E|\bE\b)\s+[A-Z]\w*"
        syn match   mechgenTraitDecl    "\v(\+T|\bT\b)\s+[A-Z]\w*"
        syn match   mechgenImplDecl     "\v\bI\b\s+[A-Z]\w*"
        syn match   mechgenModDecl      "\v\+?M\s+\w+"
        syn match   mechgenUseDecl      "\v\bu\b\s+[a-zA-Z_][\w.]*"
        syn match   mechgenVarDecl      "\v\bv\b\s+[a-zA-Z_]\w*"
        syn match   mechgenMutDecl      "\v\bm\b\s+[a-zA-Z_]\w*"

        " Keywords
        syn keyword mechgenControl      loop break continue ret yield while
        syn keyword mechgenKeyword      effect handle spec type static self Self super crate as where move unsafe extern dyn
        syn keyword mechgenBoolean      true false
        syn keyword mechgenConstant     None Some Ok Err

        " Attributes
        syn match   mechgenAttribute    "@\w\+\>"
        syn region  mechgenAttrArgs     start="@\w\+(" end=")" contains=mechgenType

        " Types
        syn keyword mechgenPrimType     i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str never
        syn match   mechgenType         "\v\b[A-Z]\w*\b"

        " Numbers
        syn match   mechgenNumber       "\v<\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?"
        syn match   mechgenHexNumber    "\v<0x[0-9a-fA-F_]+"
        syn match   mechgenOctNumber    "\v<0o[0-7_]+"
        syn match   mechgenBinNumber    "\v<0b[01_]+"

        " Operators
        syn match   mechgenOperator     "\v\=\>|\-\>|\<\-"
        syn match   mechgenOperator     "\v\=\=|\!\=|\<\=|\>\="

        " Highlighting
        hi def link mechgenComment      Comment
        hi def link mechgenCommentBlock Comment
        hi def link mechgenString       String
        hi def link mechgenPrintStr     String
        hi def link mechgenFmtStr       String
        hi def link mechgenEscape       SpecialChar
        hi def link mechgenInterp       Special
        hi def link mechgenFnDecl       Function
        hi def link mechgenStructDecl   Type
        hi def link mechgenEnumDecl     Type
        hi def link mechgenTraitDecl    Type
        hi def link mechgenImplDecl     Type
        hi def link mechgenModDecl      Include
        hi def link mechgenUseDecl      Include
        hi def link mechgenVarDecl      Identifier
        hi def link mechgenMutDecl      Identifier
        hi def link mechgenControl      Conditional
        hi def link mechgenKeyword      Keyword
        hi def link mechgenBoolean      Boolean
        hi def link mechgenConstant     Constant
        hi def link mechgenAttribute    PreProc
        hi def link mechgenAttrArgs     PreProc
        hi def link mechgenPrimType     Type
        hi def link mechgenType         Type
        hi def link mechgenNumber       Number
        hi def link mechgenHexNumber    Number
        hi def link mechgenOctNumber    Number
        hi def link mechgenBinNumber    Number
        hi def link mechgenOperator     Operator

        let b:current_syntax = 'MechGen'
      ]])
    end,
  })
end

-- ── Keymaps ─────────────────────────────────────────────────────────

function M.setup_keymaps()
  vim.api.nvim_create_autocmd('FileType', {
    pattern = 'MechGen',
    callback = function(ev)
      local opts = { buffer = ev.buf, silent = true }

      -- Build / run via mg CLI.
      vim.keymap.set('n', '<leader>rb', '<cmd>!mg build<CR>', opts)
      vim.keymap.set('n', '<leader>rr', '<cmd>!mg run<CR>', opts)
      vim.keymap.set('n', '<leader>rt', '<cmd>!mg test<CR>', opts)
      vim.keymap.set('n', '<leader>rf', '<cmd>!mg fmt<CR>', opts)
      vim.keymap.set('n', '<leader>rc', '<cmd>!mg check<CR>', opts)
    end,
  })
end

-- ── Main setup ──────────────────────────────────────────────────────

function M.setup(opts)
  opts = opts or {}
  M.setup_lsp(opts)
  M.setup_treesitter()
  M.setup_syntax()
  M.setup_keymaps()
end

return M
