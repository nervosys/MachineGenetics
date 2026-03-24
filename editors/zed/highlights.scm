; Zed tree-sitter highlight queries for MechGen.

; ── Comments ─────────────────────────────────────────────────────────
(line_comment) @comment
(block_comment) @comment

; ── Strings ──────────────────────────────────────────────────────────
(string_literal) @string
(print_string) @string.special
(format_string) @string.special
(char_literal) @string
(escape_sequence) @escape
(interpolation) @embedded

; ── Declarations ─────────────────────────────────────────────────────
(function_declaration
  keyword: _ @keyword.function
  name: (identifier) @function)

(struct_declaration
  keyword: _ @keyword
  name: (type_identifier) @type)

(enum_declaration
  keyword: _ @keyword
  name: (type_identifier) @type)

(trait_declaration
  keyword: _ @keyword
  name: (type_identifier) @type)

(impl_declaration
  keyword: _ @keyword
  type: (type_identifier) @type)

(module_declaration
  keyword: _ @keyword
  name: (identifier) @title)

(use_declaration
  keyword: _ @keyword
  path: (path) @title)

(variable_binding
  keyword: _ @keyword
  name: (identifier) @variable)

(mutable_binding
  keyword: _ @keyword
  name: (identifier) @variable)

; ── Keywords ─────────────────────────────────────────────────────────
["loop" "break" "continue" "ret" "yield" "while"] @keyword
["effect" "handle" "spec" "type" "static" "as" "where" "move"] @keyword
["self" "Self" "super" "crate"] @variable.special

; ── Attributes ───────────────────────────────────────────────────────
(attribute) @attribute

; ── Constants ────────────────────────────────────────────────────────
(boolean_literal) @boolean
["None" "Some" "Ok" "Err"] @constant

; ── Types ────────────────────────────────────────────────────────────
(primitive_type) @type.builtin
(type_identifier) @type

; ── Numbers ──────────────────────────────────────────────────────────
(integer_literal) @number
(float_literal) @number

; ── Operators ────────────────────────────────────────────────────────
(binary_operator) @operator
(unary_operator) @operator
["=>" "->" ".." "?" "@"] @operator

; ── Punctuation ──────────────────────────────────────────────────────
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" "." ":"] @punctuation.delimiter

; ── Sigils ───────────────────────────────────────────────────────────
(box_type "^" @operator)
(rc_type "$" @operator)
(arc_type "@" @operator)
(vec_type "~" @operator)
(ref_mut "&!" @operator)

; ── Parameters ───────────────────────────────────────────────────────
(parameter name: (identifier) @variable)

; ── Function calls ───────────────────────────────────────────────────
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (identifier) @function.method))
