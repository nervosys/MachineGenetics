// tree-sitter-MAGE — Tree-sitter grammar for the MAGE language.
//
// This grammar defines the concrete syntax tree structure used by
// Neovim, Helix, Zed, and other tree-sitter-aware editors.
//
// Build: npx tree-sitter generate
// Test:  npx tree-sitter test

/// <reference types="tree-sitter-cli/dsl" />

module.exports = grammar({
  name: 'MAGE',

  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  word: $ => $.identifier,

  rules: {
    // ── Top level ───────────────────────────────────────────────────
    source_file: $ => repeat($._item),

    _item: $ => choice(
      $.function_declaration,
      $.struct_declaration,
      $.enum_declaration,
      $.trait_declaration,
      $.impl_declaration,
      $.module_declaration,
      $.use_declaration,
      $.variable_binding,
      $.mutable_binding,
      $.const_declaration,
      $.effect_declaration,
      $.attribute,
      $.expression_statement,
    ),

    // ── Comments ────────────────────────────────────────────────────
    line_comment: $ => token(seq('//', /.*/)),

    block_comment: $ => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),

    // ── Functions ───────────────────────────────────────────────────
    function_declaration: $ => seq(
      field('keyword', $.function_keyword),
      field('name', $.identifier),
      optional($.generic_params),
      '(',
      optional($.parameter_list),
      ')',
      optional(seq('->', field('return_type', $._type))),
      optional($.where_clause),
      optional($.effect_annotation),
      field('body', $.block),
    ),

    function_keyword: $ => choice(
      'f', '+f', '~f', 'af', '+af', '~af',
      'c', '+c',
    ),

    parameter_list: $ => seq(
      $.parameter,
      repeat(seq(',', $.parameter)),
      optional(','),
    ),

    parameter: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $._type),
    ),

    // ── Structs ─────────────────────────────────────────────────────
    struct_declaration: $ => seq(
      field('keyword', choice('S', '+S', '~S')),
      field('name', $.type_identifier),
      optional($.generic_params),
      choice(
        $.struct_body,
        ';',
      ),
    ),

    struct_body: $ => seq(
      '{',
      optional(seq(
        $.struct_field,
        repeat(seq(',', $.struct_field)),
        optional(','),
      )),
      '}',
    ),

    struct_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $._type),
    ),

    // ── Enums ───────────────────────────────────────────────────────
    enum_declaration: $ => seq(
      field('keyword', choice('E', '+E', '~E')),
      field('name', $.type_identifier),
      optional($.generic_params),
      '{',
      optional(seq(
        $.enum_variant,
        repeat(seq(',', $.enum_variant)),
        optional(','),
      )),
      '}',
    ),

    enum_variant: $ => seq(
      field('name', $.type_identifier),
      optional(choice(
        seq('(', commaSep1($._type), ')'),
        $.struct_body,
      )),
    ),

    // ── Traits ──────────────────────────────────────────────────────
    trait_declaration: $ => seq(
      field('keyword', choice('T', '+T', '~T')),
      field('name', $.type_identifier),
      optional($.generic_params),
      optional($.where_clause),
      '{',
      repeat($._item),
      '}',
    ),

    // ── Impl ────────────────────────────────────────────────────────
    impl_declaration: $ => seq(
      'I',
      optional(seq(
        field('trait', $.type_identifier),
        '~',
      )),
      '~',
      field('type', $.type_identifier),
      optional($.generic_params),
      optional($.where_clause),
      '{',
      repeat($._item),
      '}',
    ),

    // ── Modules ─────────────────────────────────────────────────────
    module_declaration: $ => seq(
      field('keyword', choice('M', '+M')),
      field('name', $.identifier),
      choice(
        seq('{', repeat($._item), '}'),
        ';',
      ),
    ),

    // ── Use ─────────────────────────────────────────────────────────
    use_declaration: $ => seq(
      field('keyword', choice('u', '+u')),
      field('path', $.path),
      ';',
    ),

    // ── Bindings ────────────────────────────────────────────────────
    variable_binding: $ => seq(
      field('keyword', 'v'),
      field('name', $.identifier),
      optional(seq(':', field('type', $._type))),
      optional(seq('=', field('value', $._expression))),
      ';',
    ),

    mutable_binding: $ => seq(
      field('keyword', 'm'),
      field('name', $.identifier),
      optional(seq(':', field('type', $._type))),
      optional(seq('=', field('value', $._expression))),
      ';',
    ),

    const_declaration: $ => seq(
      field('keyword', choice('+v', 'v')),
      field('name', /[A-Z_][A-Z0-9_]*/),
      ':',
      field('type', $._type),
      '=',
      field('value', $._expression),
      ';',
    ),

    // ── Effects ─────────────────────────────────────────────────────
    effect_declaration: $ => seq(
      'effect',
      field('name', $.identifier),
      '{',
      repeat($.effect_operation),
      '}',
    ),

    effect_operation: $ => seq(
      'f',
      field('name', $.identifier),
      '(',
      optional($.parameter_list),
      ')',
      optional(seq('->', field('return_type', $._type))),
      ';',
    ),

    effect_annotation: $ => seq(
      '/',
      $.identifier,
      repeat(seq('+', $.identifier)),
    ),

    // ── Attributes ──────────────────────────────────────────────────
    attribute: $ => seq(
      '@',
      $.identifier,
      optional(seq('(', optional(commaSep1($._expression)), ')')),
    ),

    // ── Generics ────────────────────────────────────────────────────
    generic_params: $ => seq(
      '[',
      commaSep1($.generic_param),
      ']',
    ),

    generic_param: $ => seq(
      $.type_identifier,
      optional(seq(':', $.trait_bound)),
    ),

    trait_bound: $ => seq(
      $.type_identifier,
      repeat(seq('+', $.type_identifier)),
    ),

    where_clause: $ => seq(
      '~>',
      commaSep1($.where_predicate),
    ),

    where_predicate: $ => seq(
      $.type_identifier,
      ':',
      $.trait_bound,
    ),

    // ── Types ───────────────────────────────────────────────────────
    _type: $ => choice(
      $.primitive_type,
      $.type_identifier,
      $.generic_type,
      $.vec_type,
      $.option_type,
      $.result_type,
      $.box_type,
      $.rc_type,
      $.arc_type,
      $.ref_type,
      $.ref_mut,
      $.slice_type,
      $.map_type,
      $.set_type,
      $.function_type,
      $.tuple_type,
    ),

    primitive_type: $ => choice(
      'i8', 'i16', 'i32', 'i64', 'i128', 'isize',
      'u8', 'u16', 'u32', 'u64', 'u128', 'usize',
      'f32', 'f64', 'bool', 'char', 's', 'str', 'never',
    ),

    generic_type: $ => seq($.type_identifier, '[', commaSep1($._type), ']'),
    vec_type: $ => seq('[', $._type, ']', '~'),
    option_type: $ => seq('?', $._type),
    result_type: $ => seq('R', '[', $._type, ',', $._type, ']'),
    box_type: $ => seq('^', $._type),
    rc_type: $ => seq('$', $._type),
    arc_type: $ => seq('@', $._type),
    ref_type: $ => seq('&', $._type),
    ref_mut: $ => seq('&!', $._type),
    slice_type: $ => seq('[', $._type, ']'),
    map_type: $ => seq('{', $._type, ':', $._type, '}'),
    set_type: $ => seq('{', $._type, '}'),
    function_type: $ => seq('f', '(', optional(commaSep1($._type)), ')', '->', $._type),
    tuple_type: $ => seq('(', commaSep1($._type), ')'),

    // ── Expressions ─────────────────────────────────────────────────
    _expression: $ => choice(
      $.identifier,
      $.type_identifier,
      $.integer_literal,
      $.float_literal,
      $.string_literal,
      $.print_string,
      $.format_string,
      $.char_literal,
      $.boolean_literal,
      $.binary_expression,
      $.unary_expression,
      $.call_expression,
      $.field_expression,
      $.index_expression,
      $.block,
      $.if_expression,
      $.match_expression,
      $.for_expression,
      $.loop_expression,
      $.closure,
      $.struct_literal,
      $.array_literal,
      $.path,
    ),

    binary_expression: $ => prec.left(1, seq(
      field('left', $._expression),
      field('operator', $.binary_operator),
      field('right', $._expression),
    )),

    binary_operator: $ => choice(
      '+', '-', '*', '/', '%',
      '==', '!=', '<', '>', '<=', '>=',
      '&&', '||',
      '&', '|', '^', '<<', '>>',
      '=', '+=', '-=', '*=', '/=',
      '..',
    ),

    unary_expression: $ => prec(10, seq(
      field('operator', $.unary_operator),
      field('operand', $._expression),
    )),

    unary_operator: $ => choice('!', '-', '&', '&!', '*'),

    call_expression: $ => prec(8, seq(
      field('function', $._expression),
      '(',
      optional(commaSep1($._expression)),
      ')',
    )),

    field_expression: $ => prec(9, seq(
      field('object', $._expression),
      '.',
      field('field', $.identifier),
    )),

    index_expression: $ => prec(8, seq(
      field('object', $._expression),
      '[',
      field('index', $._expression),
      ']',
    )),

    // ── Control flow ────────────────────────────────────────────────
    if_expression: $ => seq(
      '?',
      field('condition', $._expression),
      field('consequence', $.block),
      optional(seq(':', field('alternative', choice($.block, $.if_expression)))),
    ),

    match_expression: $ => seq(
      '?',
      field('value', $._expression),
      '{',
      repeat($.match_arm),
      '}',
    ),

    match_arm: $ => seq(
      field('pattern', $._expression),
      '=>',
      field('body', choice($._expression, $.block)),
      optional(','),
    ),

    for_expression: $ => seq(
      '@',
      field('pattern', $.identifier),
      '~',
      field('iterator', $._expression),
      field('body', $.block),
    ),

    loop_expression: $ => seq('loop', $.block),

    // ── Closures ────────────────────────────────────────────────────
    closure: $ => seq(
      '|',
      optional($.parameter_list),
      '|',
      choice($._expression, $.block),
    ),

    // ── Struct literals ─────────────────────────────────────────────
    struct_literal: $ => seq(
      $.type_identifier,
      '@{',
      optional(seq(
        $.field_init,
        repeat(seq(',', $.field_init)),
        optional(','),
      )),
      '}',
    ),

    field_init: $ => seq(
      field('name', $.identifier),
      ':',
      field('value', $._expression),
    ),

    // ── Literals ────────────────────────────────────────────────────
    array_literal: $ => seq('[', optional(commaSep1($._expression)), ']', optional('~')),

    integer_literal: $ => token(choice(
      /[0-9][0-9_]*(i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize)?/,
      /0x[0-9a-fA-F_]+/,
      /0o[0-7_]+/,
      /0b[01_]+/,
    )),

    float_literal: $ => token(
      /[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9_]+)?(f32|f64)?/,
    ),

    string_literal: $ => seq('"', repeat(choice($.escape_sequence, /[^"\\]+/)), '"'),
    print_string: $ => seq('p"', repeat(choice($.escape_sequence, $.interpolation, /[^"\\{}]+/)), '"'),
    format_string: $ => seq('f"', repeat(choice($.escape_sequence, $.interpolation, /[^"\\{}]+/)), '"'),
    char_literal: $ => seq("'", choice($.escape_sequence, /[^'\\]/), "'"),

    escape_sequence: $ => token.immediate(/\\[nrt\\'"0]/),
    interpolation: $ => seq('{', $._expression, '}'),

    boolean_literal: $ => choice('1b', '0b', 'true', 'false'),

    // ── Blocks ──────────────────────────────────────────────────────
    block: $ => seq('{', repeat($._item), optional($._expression), '}'),

    expression_statement: $ => seq($._expression, ';'),

    // ── Identifiers and paths ───────────────────────────────────────
    identifier: $ => /[a-z_][a-zA-Z0-9_]*/,
    type_identifier: $ => /[A-Z][a-zA-Z0-9_]*/,
    path: $ => seq($.identifier, repeat(seq('.', $.identifier))),
  },
});

// Helper: comma-separated list (at least one element).
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
