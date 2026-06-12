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
    rdx = 'MechGen',
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
        syn match   redoxComment      "//.*$"
        syn region  redoxCommentBlock start="/\*" end="\*/"

        " Strings
        syn region  redoxString       start='"' end='"' contains=redoxEscape,redoxInterp
        syn region  redoxPrintStr     start='p"' end='"' contains=redoxEscape,redoxInterp
        syn region  redoxFmtStr       start='f"' end='"' contains=redoxEscape,redoxInterp
        syn match   redoxEscape       contained "\\."
        syn region  redoxInterp       contained start="{" end="}"

        " Declarations
        syn match   redoxFnDecl       "\v(\+f|~f|\baf\b|\bf\b)\s+[a-zA-Z_]\w*"
        syn match   redoxStructDecl   "\v(\+S|\bS\b)\s+[A-Z]\w*"
        syn match   redoxEnumDecl     "\v(\+E|\bE\b)\s+[A-Z]\w*"
        syn match   redoxTraitDecl    "\v(\+T|\bT\b)\s+[A-Z]\w*"
        syn match   redoxImplDecl     "\v\bI\b\s+[A-Z]\w*"
        syn match   redoxModDecl      "\v\+?M\s+\w+"
        syn match   redoxUseDecl      "\v\bu\b\s+[a-zA-Z_][\w.]*"
        syn match   redoxVarDecl      "\v\bv\b\s+[a-zA-Z_]\w*"
        syn match   redoxMutDecl      "\v\bm\b\s+[a-zA-Z_]\w*"

        " Keywords
        syn keyword redoxControl      loop break continue ret yield while
        syn keyword redoxKeyword      effect handle spec type static self Self super crate as where move unsafe extern dyn
        syn keyword redoxBoolean      true false
        syn keyword redoxConstant     None Some Ok Err

        " Attributes
        syn match   redoxAttribute    "@\w\+\>"
        syn region  redoxAttrArgs     start="@\w\+(" end=")" contains=redoxType

        " Types
        syn keyword redoxPrimType     i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str never
        syn match   redoxType         "\v\b[A-Z]\w*\b"

        " Numbers
        syn match   redoxNumber       "\v<\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?"
        syn match   redoxHexNumber    "\v<0x[0-9a-fA-F_]+"
        syn match   redoxOctNumber    "\v<0o[0-7_]+"
        syn match   redoxBinNumber    "\v<0b[01_]+"

        " Operators
        syn match   redoxOperator     "\v\=\>|\-\>|\<\-"
        syn match   redoxOperator     "\v\=\=|\!\=|\<\=|\>\="

        " Highlighting
        hi def link redoxComment      Comment
        hi def link redoxCommentBlock Comment
        hi def link redoxString       String
        hi def link redoxPrintStr     String
        hi def link redoxFmtStr       String
        hi def link redoxEscape       SpecialChar
        hi def link redoxInterp       Special
        hi def link redoxFnDecl       Function
        hi def link redoxStructDecl   Type
        hi def link redoxEnumDecl     Type
        hi def link redoxTraitDecl    Type
        hi def link redoxImplDecl     Type
        hi def link redoxModDecl      Include
        hi def link redoxUseDecl      Include
        hi def link redoxVarDecl      Identifier
        hi def link redoxMutDecl      Identifier
        hi def link redoxControl      Conditional
        hi def link redoxKeyword      Keyword
        hi def link redoxBoolean      Boolean
        hi def link redoxConstant     Constant
        hi def link redoxAttribute    PreProc
        hi def link redoxAttrArgs     PreProc
        hi def link redoxPrimType     Type
        hi def link redoxType         Type
        hi def link redoxNumber       Number
        hi def link redoxHexNumber    Number
        hi def link redoxOctNumber    Number
        hi def link redoxBinNumber    Number
        hi def link redoxOperator     Operator

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

      -- Build / run via rdx CLI.
      vim.keymap.set('n', '<leader>rb', '<cmd>!rdx build<CR>', opts)
      vim.keymap.set('n', '<leader>rr', '<cmd>!rdx run<CR>', opts)
      vim.keymap.set('n', '<leader>rt', '<cmd>!rdx test<CR>', opts)
      vim.keymap.set('n', '<leader>rf', '<cmd>!rdx fmt<CR>', opts)
      vim.keymap.set('n', '<leader>rc', '<cmd>!rdx check<CR>', opts)
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
