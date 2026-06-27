; Functions
(function_declaration
  name: (identifier) @name) @item

; Structs
(struct_declaration
  name: (type_identifier) @name) @item

; Enums
(enum_declaration
  name: (type_identifier) @name) @item

; Impl blocks
(impl_declaration
  type: (type_identifier) @name) @item
