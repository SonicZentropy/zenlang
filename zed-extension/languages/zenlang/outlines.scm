; Outline queries - match named definitions

; Functions
(function_declaration (identifier) @_function)

; Structs
(struct_declaration (type_identifier) @_type)

; Enums
(enum_declaration (type_identifier) @_type)

; Impl blocks
(impl_declaration (type_identifier) @_type)

; Constants/variables
(let_statement (identifier) @_variable)

; Types
(type_identifier) @_type