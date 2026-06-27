; Outline queries - match named definitions

; Functions
(function_declaration (identifier) @function)

; Structs
(struct_declaration (type_identifier) @type)

; Enums
(enum_declaration (type_identifier) @type)

; Impl blocks
(impl_declaration (type_identifier) @type)

; Constants/variables
(let_statement (identifier) @variable)

; Types
(type_identifier) @type