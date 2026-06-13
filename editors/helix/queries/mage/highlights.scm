; Helix tree-sitter highlight queries for MAGE.
;
; These queries map tree-sitter node types to Helix highlight groups.
; Requires a tree-sitter-MAGE parser to be installed.

; ── Comments ─────────────────────────────────────────────────────────
(line_comment) @comment.line
(block_comment) @comment.block

; ── Strings ──────────────────────────────────────────────────────────
(string_literal) @string
(print_string) @string.special
(format_string) @string.special
(char_literal) @constant.character
(escape_sequence) @constant.character.escape
(interpolation) @variable

; ── Declarations ─────────────────────────────────────────────────────
(function_declaration
  keyword: _ @keyword.function
  name: (identifier) @function)

(struct_declaration
  keyword: _ @keyword.storage.type
  name: (type_identifier) @type)

(enum_declaration
  keyword: _ @keyword.storage.type
  name: (type_identifier) @type)

(trait_declaration
  keyword: _ @keyword.storage.type
  name: (type_identifier) @type)

(impl_declaration
  keyword: _ @keyword.storage.type
  type: (type_identifier) @type)

(module_declaration
  keyword: _ @keyword.control.import
  name: (identifier) @namespace)

(use_declaration
  keyword: _ @keyword.control.import
  path: (path) @namespace)

(variable_binding
  keyword: _ @keyword
  name: (identifier) @variable)

(mutable_binding
  keyword: _ @keyword
  name: (identifier) @variable.other.member)

; ── Keywords ─────────────────────────────────────────────────────────
[
  "loop"
  "break"
  "continue"
  "ret"
  "yield"
  "while"
] @keyword.control

[
  "effect"
  "handle"
  "spec"
  "type"
  "static"
  "as"
  "where"
  "move"
] @keyword

[
  "self"
  "Self"
  "super"
  "crate"
] @variable.builtin

; ── Attributes ───────────────────────────────────────────────────────
(attribute) @attribute

; ── Constants ────────────────────────────────────────────────────────
(boolean_literal) @constant.builtin.boolean
[
  "None"
  "Some"
  "Ok"
  "Err"
] @constant.builtin

; ── Types ────────────────────────────────────────────────────────────
(primitive_type) @type.builtin
(type_identifier) @type

; ── Numbers ──────────────────────────────────────────────────────────
(integer_literal) @constant.numeric.integer
(float_literal) @constant.numeric.float

; ── Operators ────────────────────────────────────────────────────────
(binary_operator) @operator
(unary_operator) @operator
"=>" @operator
"->" @operator
".." @operator
"?" @keyword.control.conditional
"@" @punctuation.special

; ── Punctuation ──────────────────────────────────────────────────────
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
"," @punctuation.delimiter
";" @punctuation.delimiter
"." @punctuation.delimiter
":" @punctuation.delimiter

; ── Sigils ───────────────────────────────────────────────────────────
(box_type "^" @punctuation.special)
(rc_type "$" @punctuation.special)
(arc_type "@" @punctuation.special)
(vec_type "~" @punctuation.special)
(ref_mut "&!" @punctuation.special)

; ── Parameters ───────────────────────────────────────────────────────
(parameter
  name: (identifier) @variable.parameter)

; ── Function calls ───────────────────────────────────────────────────
(call_expression
  function: (identifier) @function)

(call_expression
  function: (field_expression
    field: (identifier) @function.method))
