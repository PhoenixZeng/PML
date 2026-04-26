# PML Grammar (EBNF)

[中文规范](./PML-format-spec_CN.md) | [English Spec](./PML-format-spec.md)

This file gives a compact grammar-oriented description of PML.
The EBNF below describes control-line syntax and name/type tokens.
Block boundary selection is defined in the semantic rules after the grammar.

## EBNF

```ebnf
document          = { blank_line | block } ;

block             = opening_header newline body ;

opening_header    = "[" name [ ":" [ type ] ] "]" ;
closing_header    = "[/" name "]" ;

name              = segment { "." segment } [ "#" segment ] ;
segment           = name_start { name_continue } ;

type              = type_char { type_char } ;

blank_line        = { space | tab } newline ;
newline           = "\n" ;

name_start        = unicode_letter
                  | unicode_digit
                  | "_" ;

name_continue     = unicode_letter
                  | unicode_digit
                  | unicode_mark
                  | "_" ;

type_char         = ascii_letter
                  | ascii_digit
                  | "_"
                  | "-" ;

space             = " " ;
tab               = "\t" ;
```

## Lexical Notes

1. Control syntax is recognized only when it occupies a whole line.
2. Implementations should normalize `\r\n` and `\r` to `\n`, or strip trailing `\r` before matching a control line.
3. `:type` is an optional format hint on opening headers.
4. If `:type` is omitted, or the header is written as `[NAME:]`, the parsed `type` field is the empty string.
5. Non-empty `type` values are compared case-insensitively after normalization.
6. The only built-in type alias is `md = markdown`.
7. Closing headers never include `type`; `[/NAME:TYPE]` is not a valid closing header.

## Boundary Semantics

1. After reading an opening header, search forward for a closing header with exactly the same `name`.
2. If a matching closing header exists, the block uses an explicit boundary.
   The body is everything between the opening and closing headers.
3. If no matching closing header exists, the block uses an implicit boundary.
   The body ends before the next legal opening header or at EOF.
4. Explicit boundaries have priority over implicit boundaries.
5. Body text is preserved as a raw string first.
   PML does not parse body content according to `type`.
6. PML parses one layer at a time.
   PML-looking lines inside an explicit-boundary body do not participate in current-layer parsing.
7. The separator newline before the ending boundary is not part of the body.

## Invalid Control Lines

These are invalid control lines:

```text
[]
[A key="value"]
[/A extra]
[/A:yaml]
```

These are valid opening headers:

```text
[A]
[A:]
[A:yaml]
[A#outer:toml]
```

These are valid closing headers:

```text
[/A]
[/A#outer]
```
