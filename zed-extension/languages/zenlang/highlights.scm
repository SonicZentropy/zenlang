; Keywords - match parent nodes that contain keywords
(let_statement) @keyword
(function_declaration) @keyword
(if_expression) @keyword
(match_expression) @keyword
(return_statement) @keyword
(struct_declaration) @keyword
(enum_declaration) @keyword
(impl_declaration) @keyword
(while_loop) @keyword
(for_loop) @keyword
(bool_literal) @keyword
(nil_literal) @keyword

; Types
(type_identifier) @type
(struct_declaration (type_identifier) @type)
(enum_declaration (type_identifier) @type)

; Functions
(function_declaration (identifier) @function)
(call_expression (identifier) @function)

; Parameters
(parameter (identifier) @variable.parameter)

; Variables
(identifier) @variable

; String literals
(string_literal) @string

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
