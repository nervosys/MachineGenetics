-- MAGE language support for Neovim.
--
-- Installation:
--   1. Copy this file to ~/.config/nvim/lua/MAGE.lua
--   2. Add `require('MAGE')` to your init.lua
--
-- Or with lazy.nvim, add this directory as a local plugin.

local M = {}

-- ── Filetype detection ──────────────────────────────────────────────

vim.filetype.add({
  extension = {
    mg = 'MAGE',
  },
  filename = {
    ['Forge.toml'] = 'toml',
  },
})

-- ── Formatting ──────────────────────────────────────────────────────
--
-- `gq` and `:normal gggqG` run this; it needs no language server, because
-- `mage-parse --fmt-compact -` reads the buffer from standard input and writes
-- the formatted source to stdout. That stdin mode did not exist until
-- 2026-09-02 (open item 25) — before it, `-` was opened as a *file*, which is
-- why nothing here formatted despite `--fmt-compact` having worked all along.
--
-- `opts.fmt_cmd` overrides it; `opts.fmt = false` leaves `formatprg` alone.

vim.api.nvim_create_autocmd('FileType', {
  pattern = 'MAGE',
  callback = function(ev)
    if M._fmt == false then
      return
    end
    vim.bo[ev.buf].formatprg = M._fmt_cmd or 'mage-parse --fmt-compact -'
  end,
})

-- ── LSP (RAP) ───────────────────────────────────────────────────────
--
-- WARNING (checked 2026-08-25): this registration cannot work as written.
--   1. RAP is not LSP. `prototype/src/rap.rs` has zero occurrences of
--      `initialize` or `textDocument`; it speaks `language/parse` and 36
--      other custom methods, so the LSP handshake never completes.
--   2. There is no `rap` binary. The server is `mage-parse --rap`.
--   3. RAP listens on TCP (127.0.0.1:9876 by default); `cmd` below spawns a
--      process and speaks over stdio.
-- The settings block names completion/inlayHints/diagnostics capabilities RAP
-- has no methods for. Tree-sitter highlighting (above) does work.
-- See internals/07-rap-server.md §7.6.

function M.setup_lsp(opts)
  opts = opts or {}

  -- Without an explicit `rap_cmd`, this would register `cmd = { 'rap' }` — a
  -- binary that does not exist, speaking a protocol RAP does not implement,
  -- over a transport it does not use. Neovim's failure for that arrives later
  -- and elsewhere: a spawn error in `:LspLog`, from a server the user did not
  -- knowingly configure. Say it here instead, where the call was made.
  --
  -- `rap_cmd` is still honoured, because supplying your own LSP shim is the
  -- one way this function can do something useful today. Passing it is a
  -- statement that you have one.
  if not opts.rap_cmd then
    vim.notify(
      'MAGE: no LSP server is available. RAP is an agent protocol (JSON-RPC over '
        .. 'TCP, 37 custom methods), not a language server: it implements no '
        .. '`initialize` and no `textDocument/*`, and there is no `rap` binary — '
        .. 'the server is `mage-parse --rap`. Tree-sitter highlighting works and '
        .. 'is set up separately by require("mage").setup(). To use your own LSP '
        .. 'shim over RAP, pass rap_cmd (a string or an argv table). '
        .. 'See internals/07-rap-server.md §7.6.',
      vim.log.levels.WARN
    )
    return
  end

  local lspconfig_ok, lspconfig = pcall(require, 'lspconfig')
  if not lspconfig_ok then
    vim.notify('MAGE: nvim-lspconfig not found', vim.log.levels.WARN)
    return
  end

  local configs = require('lspconfig.configs')

  -- Register RAP as a custom LSP server if not already defined.
  if not configs.rap then
    configs.rap = {
      default_config = {
        -- `rap_cmd` was a bare string here before, so both shapes are taken:
        -- changing an option's contract to fix a warning would be its own
        -- small version of this file's problem.
        cmd = type(opts.rap_cmd) == 'table' and opts.rap_cmd or { opts.rap_cmd },
        filetypes = { 'MAGE' },
        root_dir = lspconfig.util.root_pattern('Forge.toml', '.git'),
        -- No `settings` block.
        --
        -- This carried `checkOnSave`, `diagnostics.skb`, `diagnostics.effects`,
        -- `completion.autoimport`, `completion.sigils`, `inlayHints.typeHints`,
        -- `inlayHints.effectHints` and `inlayHints.costHints` -- eight options
        -- for capabilities RAP has no methods for, invented wholesale. A user
        -- pointing `rap_cmd` at their own shim would have had MAGE's imaginary
        -- configuration sent to it as if it meant something.
        --
        -- Pass your shim's real settings through `opts.lsp` instead; whatever
        -- you put there reaches `lspconfig` untouched.
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
  parser_config.MAGE = {
    install_info = {
      -- When a tree-sitter-MAGE parser is published, point url here.
      url = 'https://github.com/nervosys/tree-sitter-MAGE',
      files = { 'src/parser.c' },
      branch = 'main',
    },
    filetype = 'MAGE',
  }
end

-- ── Syntax (fallback vim regex, used when tree-sitter is unavailable) ──

function M.setup_syntax()
  vim.api.nvim_create_autocmd('FileType', {
    pattern = 'MAGE',
    callback = function()
      local buf = vim.api.nvim_get_current_buf()

      -- Comments.
      vim.bo[buf].commentstring = '// %s'

      -- Basic syntax groups via vim.cmd (fallback if no tree-sitter).
      vim.cmd([[
        if exists('b:current_syntax') | finish | endif

        " Comments
        syn match   mageComment      "//.*$"
        syn region  mageCommentBlock start="/\*" end="\*/"

        " Strings
        syn region  mageString       start='"' end='"' contains=mageEscape,mageInterp
        syn region  magePrintStr     start='p"' end='"' contains=mageEscape,mageInterp
        syn region  mageFmtStr       start='f"' end='"' contains=mageEscape,mageInterp
        syn match   mageEscape       contained "\\."
        syn region  mageInterp       contained start="{" end="}"

        " Declarations
        syn match   mageFnDecl       "\v(\+f|~f|\baf\b|\bf\b)\s+[a-zA-Z_]\w*"
        syn match   mageStructDecl   "\v(\+S|\bS\b)\s+[A-Z]\w*"
        syn match   mageEnumDecl     "\v(\+E|\bE\b)\s+[A-Z]\w*"
        syn match   mageTraitDecl    "\v(\+T|\bT\b)\s+[A-Z]\w*"
        syn match   mageImplDecl     "\v\bI\b\s+[A-Z]\w*"
        syn match   mageModDecl      "\v\+?M\s+\w+"
        syn match   mageUseDecl      "\v\bu\b\s+[a-zA-Z_][\w.]*"
        syn match   mageVarDecl      "\v\bv\b\s+[a-zA-Z_]\w*"
        syn match   mageMutDecl      "\v\bm\b\s+[a-zA-Z_]\w*"

        " Keywords
        syn keyword mageControl      loop break continue ret yield while
        syn keyword mageKeyword      effect handle spec type static self Self super crate as where move unsafe extern dyn
        syn keyword mageBoolean      true false
        syn keyword mageConstant     None Some Ok Err

        " Attributes
        syn match   mageAttribute    "@\w\+\>"
        syn region  mageAttrArgs     start="@\w\+(" end=")" contains=mageType

        " Types
        syn keyword magePrimType     i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str never
        syn match   mageType         "\v\b[A-Z]\w*\b"

        " Numbers
        syn match   mageNumber       "\v<\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?"
        syn match   mageHexNumber    "\v<0x[0-9a-fA-F_]+"
        syn match   mageOctNumber    "\v<0o[0-7_]+"
        syn match   mageBinNumber    "\v<0b[01_]+"

        " Operators
        syn match   mageOperator     "\v\=\>|\-\>|\<\-"
        syn match   mageOperator     "\v\=\=|\!\=|\<\=|\>\="

        " Highlighting
        hi def link mageComment      Comment
        hi def link mageCommentBlock Comment
        hi def link mageString       String
        hi def link magePrintStr     String
        hi def link mageFmtStr       String
        hi def link mageEscape       SpecialChar
        hi def link mageInterp       Special
        hi def link mageFnDecl       Function
        hi def link mageStructDecl   Type
        hi def link mageEnumDecl     Type
        hi def link mageTraitDecl    Type
        hi def link mageImplDecl     Type
        hi def link mageModDecl      Include
        hi def link mageUseDecl      Include
        hi def link mageVarDecl      Identifier
        hi def link mageMutDecl      Identifier
        hi def link mageControl      Conditional
        hi def link mageKeyword      Keyword
        hi def link mageBoolean      Boolean
        hi def link mageConstant     Constant
        hi def link mageAttribute    PreProc
        hi def link mageAttrArgs     PreProc
        hi def link magePrimType     Type
        hi def link mageType         Type
        hi def link mageNumber       Number
        hi def link mageHexNumber    Number
        hi def link mageOctNumber    Number
        hi def link mageBinNumber    Number
        hi def link mageOperator     Operator

        let b:current_syntax = 'MAGE'
      ]])
    end,
  })
end

-- ── Keymaps ─────────────────────────────────────────────────────────

function M.setup_keymaps()
  vim.api.nvim_create_autocmd('FileType', {
    pattern = 'MAGE',
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

  M._fmt = opts.fmt
  M._fmt_cmd = opts.fmt_cmd

  -- LSP is opt-in now. `setup_lsp()` explains that there is no language server
  -- and returns, which is the right answer when someone asks for one — and the
  -- wrong thing to say to everyone who called `setup()` for highlighting.
  -- Making it unconditional would have turned an accurate message into noise
  -- on every MAGE buffer, which is how accurate messages get ignored.
  if opts.rap_cmd or opts.lsp then
    M.setup_lsp(opts)
  end

  M.setup_treesitter()
  M.setup_syntax()
  M.setup_keymaps()
end

return M
