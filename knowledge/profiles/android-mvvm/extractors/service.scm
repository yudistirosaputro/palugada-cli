; A class/interface whose declared name ends in "Service".
; Kotlin interfaces parse as class_declaration, so this matches both.
(class_declaration name: (identifier) @name (#match? @name "Service$"))
