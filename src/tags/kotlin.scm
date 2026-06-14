; Generic symbol tags for Kotlin — one capture per definition kind.
; The capture name is the symbol kind; the captured node is the symbol name.
; (interface and enum class also parse as class_declaration → reported as class.)
(class_declaration name: (identifier) @class)
(object_declaration name: (identifier) @object)
(function_declaration name: (identifier) @function)
(property_declaration (variable_declaration (identifier) @property))
