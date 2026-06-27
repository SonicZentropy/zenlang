; Keywords
[
  "let"
  "mut"
  "fn"
  "if"
  "else"
  "while"
  "for"
  "in"
  "match"
  "return"
  "true"
  "false"
  "nil"
  "struct"
  "enum"
  "impl"
  "self"
  "as"
] @keyword

; Types
(type_identifier) @type
(struct_declaration name: (type_identifier) @type)
(enum_declaration name: (type_identifier) @type)

; Functions
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)

; Parameters
(parameter name: (identifier) @variable.parameter)

; Variables
(identifier) @variable

; String literals
(string_literal) @string
(escape_sequence) @string.escape

; Number literals
(number_literal) @number
(float_literal) @number

; Comments
(comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "."
  ".."
] @operator

; Brackets
[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

; Delimiters
[
  ","
  ";"
  ":"
  "->"
] @punctuation.delimiter
