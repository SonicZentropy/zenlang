; Function bodies
(function_declaration
  (block) @function.inside) @function.around

; If blocks
(if_expression
  (block) @function.inside) @function.around

; For loops
(for_loop
  (block) @function.inside) @function.around

; While loops
(while_loop
  (block) @function.inside) @function.around
