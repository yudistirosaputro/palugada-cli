; Generic symbol tags for Rust — one capture per definition kind.
; The capture name is the symbol kind; the captured node is the symbol name.
(struct_item name: (type_identifier) @struct)
(enum_item name: (type_identifier) @enum)
(union_item name: (type_identifier) @struct)
(trait_item name: (type_identifier) @trait)
(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(const_item name: (identifier) @const)
(static_item name: (identifier) @const)
(type_item name: (type_identifier) @type)
(mod_item name: (identifier) @module)
(macro_definition name: (identifier) @macro)
