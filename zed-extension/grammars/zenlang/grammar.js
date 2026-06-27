/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: "zenlang",

  extras: ($) => [/\s/, $.comment],

  conflicts: ($) => [],

  word: ($) => $.identifier,

  rules: {
    source_file: ($) => repeat($._definition),

    _definition: ($) => choice(
      $.function_declaration,
      $.struct_declaration,
      $.enum_declaration,
      $.impl_declaration,
      $.let_statement,
      $.expression_statement,
    ),

    // ── Comments ──
    comment: ($) => token(choice(
      seq("//", /[^\n]*/),
      seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/"),
    )),

    // ── Identifiers ──
    identifier: ($) => /[a-z_][a-zA-Z0-9_]*/,
    type_identifier: ($) => /[A-Z][a-zA-Z0-9_]*/,

    // ── Literals ──
    number_literal: ($) => token(choice(
      seq(choice("0x", "0X"), /[0-9a-fA-F_]+/),
      /[0-9][0-9_]*/,
    )),
    float_literal: ($) => token(seq(
      /[0-9][0-9_]*/,
      ".",
      /[0-9][0-9_]*/,
      optional(seq(choice("e", "E"), optional(choice("+", "-")), /[0-9][0-9_]*/)),
    )),
    string_literal: ($) => token(choice(
      seq('"', /[^"\\]*(\\[\s\S][^"\\]*)*/, '"'),
      seq("'", /[^'\\]*(\\[\s\S][^'\\]*)*/, "'"),
    )),
    bool_literal: ($) => choice("true", "false"),
    nil_literal: ($) => "nil",

    _literal: ($) => choice(
      $.nil_literal,
      $.bool_literal,
      $.float_literal,
      $.number_literal,
      $.string_literal,
    ),

    // ── Primary expressions (leaf nodes) ──
    _primary_expression: ($) => choice(
      $.identifier,
      $._literal,
      $.parenthesized_expression,
      $.block,
      $.array_literal,
      $.struct_expression,
    ),

    parenthesized_expression: ($) => seq("(", $._expression, ")"),

    // ── Postfix / call expressions (wrap primaries) ──
    _postfix_expression: ($) => choice(
      $.call_expression,
      $.field_expression,
      $.index_expression,
    ),

    call_expression: ($) => prec(1, seq(
      choice($.identifier, $.field_expression, $.parenthesized_expression),
      $.arguments,
    )),

    arguments: ($) => seq("(", comma_sep($._expression), ")"),

    field_expression: ($) => prec(2, seq(
      choice($.identifier, $.call_expression, $.parenthesized_expression),
      ".",
      $.identifier,
    )),

    index_expression: ($) => prec(1, seq(
      choice($.identifier, $.call_expression, $.parenthesized_expression),
      "[",
      $._expression,
      "]",
    )),

    // ── Full expression (includes everything) ──
    _expression: ($) => choice(
      $.binary_expression,
      $.unary_expression,
      $.if_expression,
      $.match_expression,
      $.for_loop,
      $.while_loop,
      $._postfix_expression,
      $._primary_expression,
    ),

    binary_expression: ($) => prec.left(seq(
      $._expression,
      choice("+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "..", "="),
      $._expression,
    )),

    unary_expression: ($) => prec(3, seq(
      choice("-", "!"),
      $._expression,
    )),

    // ── Control flow ──
    if_expression: ($) => prec.right(seq(
      "if",
      $._expression,
      $.block,
      optional(seq("else", choice($.if_expression, $.block))),
    )),

    match_expression: ($) => seq(
      "match",
      $._expression,
      "{",
      repeat($.match_arm),
      "}",
    ),

    match_arm: ($) => seq(
      choice($._literal, $.identifier, "_"),
      "=>",
      $._expression,
      optional(","),
    ),

    block: ($) => seq(
      "{",
      repeat(choice(
        $.let_statement,
        $.expression_statement,
      )),
      optional($._expression),
      "}",
    ),

    // ── Statements ──
    let_statement: ($) => prec(-1, seq(
      "let",
      optional("mut"),
      $.identifier,
      optional(seq(":", $.type)),
      optional(seq("=", $._expression)),
      optional(";"),
    )),

    expression_statement: ($) => prec(-1, seq(
      $._expression,
      optional(";"),
    )),

    // ── Functions ──
    function_declaration: ($) => prec(-1, seq(
      "fn",
      $.identifier,
      $.parameters,
      choice(
        $.block,
        seq("->", $.type),
      ),
    )),

    parameters: ($) => seq(
      "(",
      comma_sep($.parameter),
      ")",
    ),

    parameter: ($) => seq(
      $.identifier,
      optional(seq(":", $.type)),
    ),

    // ── Structs ──
    struct_declaration: ($) => seq(
      "struct",
      $.type_identifier,
      "{",
      comma_sep($.field),
      "}",
    ),

    field: ($) => seq(
      $.identifier,
      optional(seq(":", $.type)),
    ),

    struct_expression: ($) => seq(
      $.type_identifier,
      "{",
      comma_sep($.field_initializer),
      "}",
    ),

    field_initializer: ($) => seq(
      $.identifier,
      ":",
      $._expression,
    ),

    // ── Enums ──
    enum_declaration: ($) => seq(
      "enum",
      $.type_identifier,
      "{",
      comma_sep($.enum_variant),
      "}",
    ),

    enum_variant: ($) => seq(
      $.type_identifier,
      optional(seq("(", comma_sep($.type), ")")),
    ),

    // ── Impl blocks ──
    impl_declaration: ($) => seq(
      "impl",
      $.type_identifier,
      "{",
      repeat($.function_declaration),
      "}",
    ),

    // ── Arrays ──
    array_literal: ($) => seq(
      "[",
      comma_sep($._expression),
      "]",
    ),

    // ── Type annotations ──
    type: ($) => choice(
      $.type_identifier,
      $.array_type,
    ),

    array_type: ($) => seq("[", $.type, "]"),

    // ── Loops ──
    for_loop: ($) => prec(-1, seq(
      "for",
      $.identifier,
      "in",
      $._expression,
      $.block,
    )),

    while_loop: ($) => prec(-1, seq(
      "while",
      $._expression,
      $.block,
    )),
  },
});

function comma_sep(rule) {
  return sep(rule, ",");
}

function sep(rule, separator) {
  return optional(seq(rule, repeat(seq(separator, rule)), optional(separator)));
}
