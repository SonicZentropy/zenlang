; Function bodies
(function_declaration
    body: (_
        "{"
        (_)* @function.inside
        "}")) @function.around

; If / else blocks
(if_expression
    consequence: (_
        "{"
        (_)* @function.inside
        "}")) @function.around

(if_expression
    alternative: (_
        "{"
        (_)* @function.inside
        "}")) @function.around

; For loops
(for_loop
    body: (_
        "{"
        (_)* @function.inside
        "}")) @function.around

; While loops
(while_loop
    body: (_
        "{"
        (_)* @function.inside
        "}")) @function.around
