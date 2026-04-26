# PML Format Specification

[中文规范](./PML-format-spec_CN.md) | [English README](./README.md) | [中文 README](./README_CN.md)

## 1. What PML Is

PML is a lightweight block format for LLM workflows, prompt engineering, and structured text composition.

The formal name can be read as **Prompt Meta Language**.

It also keeps some intentionally open-ended interpretations, such as:

- **Peer as a Markup Language**
- **Patchwork Mosaic Language**
- **Prompt Modular Language**
- **PML forMat Language**

That ambiguity is a feature, not a bug.

PML is meant for documents that combine multiple text formats:

- system prompts
- user prompts
- config fragments
- Markdown notes
- JSON or YAML payloads

Its goal is not to replace JSON, YAML, or Markdown. Instead, it acts as a thin envelope:

1. split long text into named blocks
2. attach an optional type to each body
3. preserve the body as raw text during parsing
4. make the document readable for humans and stable for programs and LLM systems to process

PML is especially useful for prompt templates, agent payloads, and documents that need “split first, interpret later”.

## 2. Minimal Example

```text
[SYSTEM]
You are a rigorous assistant.

[CONFIG:yaml]
lang: en
tone: concise
```

After parsing, you can first think of it as this path tree model sketch:

```json
{
  "SYSTEM": {
    "type": "",
    "content": "You are a rigorous assistant."
  },
  "CONFIG": {
    "type": "yaml",
    "content": "lang: en\ntone: concise"
  }
}
```

The key point is:
`content` is returned as the original string. PML does not auto-parse YAML, JSON, or any other embedded format.
If you need deeper interpretation, do it separately.

## 3. Core Principles

PML has four core principles:

1. The document is an ordered block sequence, not a tree.
2. Parsing happens one layer at a time.
3. Each block body is first treated as raw text.
4. Block names may carry segments and suffix markers, but the core format does not prescribe tree semantics.

That means:

```text
[A.B]
[A.1]
[1.1.1]
```

are all just different block names in the core format.
PML does not automatically project them into objects, arrays, or a chapter tree.

## 4. Syntax

### 4.1 Opening Block

The basic opening header structure is:

```text
[NAME:TYPE]
```

`:TYPE` is an optional format hint.
If `:TYPE` is omitted, the parser records the type field as an empty string.
PML does not parse the body by `TYPE`; the body is always preserved as a raw string first.

### 4.2 Closing Block

The basic closing header structure is:

```text
[/NAME]
```

Closing blocks are optional.
They are only needed when the author wants an explicit boundary.

### 4.3 Name

`NAME` is the block name.
It is recommended to use uppercase or clear title-style naming.
It may contain dot segments and may end with a `#...` suffix.

Name regex approximation:

```text
[A-Za-z0-9_\u4e00-\u9fff]+(\.[A-Za-z0-9_\u4e00-\u9fff]+)*(#[A-Za-z0-9_\u4e00-\u9fff]+)?
```

Examples:

```text
A
SYSTEM
PROMPT.INPUT
a.b.c
章节.一
系统提示
A.1
1.1.1
A#raw
系统提示#outer
PROMPT.INPUT#outer
```

The `#...` suffix is part of the name for now.

For a more complete Unicode interpretation:

```text
NAME = SEGMENT ("." SEGMENT)* ("#" SEGMENT)?
```

`SEGMENT` rules:

1. first character: Unicode letter, Unicode digit, or `_`
2. following characters: Unicode letter, Unicode digit, Unicode combining mark, or `_`
3. `#` uses the same rules and does not relax anything

More formally, a complete implementation should reject:

1. punctuation
2. whitespace and spacing characters
3. control characters
4. symbols such as emoji, trademarks, and math symbols
5. private-use, surrogate, and unassigned code points

If a language exposes Unicode categories easily, implement by category.
Otherwise, the short regex above is a practical approximation.

### 4.4 Type

`TYPE` is an optional body-format tag.

Rules:

1. `:TYPE` in the opening header may be omitted
2. Type matching is case-insensitive
3. Normalize to lowercase before comparing
4. The only built-in alias is `md = markdown`
5. Any body format is allowed, including custom ones
6. `TYPE` may only contain ASCII letters, digits, `_`, and `-`

Recommended naming style: lowercase.
For semantically obvious content, omitting the type usually does not hurt human or LLM understanding.
When parsed into a programmatic model, a missing type is recorded as an empty string.
When the document is read by LLMs or rendered by tools, writing `:TYPE` is still recommended.

Common examples: `text`, `markdown`, `json`, `yaml`, `toml`, `ini`, `prompt-template`

## 5. Blocks and Boundaries

PML has one basic block structure: opening header plus body.

```text
[NAME]
body
```

The end boundary can be determined in two ways:

1. implicit boundary: when no matching closing block exists, the body ends at the next legal opening header or EOF
2. explicit boundary: when a matching closing block `[/NAME]` exists, the body ends before that closing block

Explicit boundaries have priority over implicit boundaries.
That means after reading an opening header, the parser first looks for a matching `[/NAME]`.
If found, everything between the opening and closing headers is body content; other PML-like lines inside that body do not participate in current-layer parsing.
If not found, the parser uses the implicit boundary.

## 6. Parsing Rules

### 6.1 Control Lines Must Stand Alone

PML recognizes a line as a control line only if the whole line matches the control syntax.

Recommended implementation: strip trailing `\r` first, then match the whole line.

### 6.2 Body Boundaries and Trailing Newlines

The body is made of body lines.

The body starts on the first line after the opening header and ends at:

1. the matching closing header
2. the next legal opening header
3. or EOF

The separator newline before the boundary is not part of the body.
In other words, the control line itself and the newline before it are part of the block boundary, not the body.

Therefore:

```text
[A]
hello
[/A]
```

The body of `A` is `hello`.

```text
[A]
hello

[/A]
```

The body of `A` is `hello\n`.

Empty body:

```text
[A]
[/A]
```

The body is an empty string.

One blank-line body:

```text
[A]

[/A]
```

The body is `\n`.

The same rule applies at EOF:

```text
[A]
hello
```

If the file ends immediately after `hello`, the body is `hello`.

```text
[A]
hello

```

If the file ends after the blank line, the body is `hello\n`.

This rule applies to all body types.

### 6.3 Explicit Boundaries Have Priority

If a matching closing block exists, the block uses an explicit boundary and consumes the entire middle section.

### 6.4 No Matching Close Means Implicit Boundary

If no matching closing block is found, the block uses an implicit boundary and ends at the next legal opening header or EOF.

### 6.5 Closing Block Match Rules

Two rules:

1. `NAME` must match exactly
2. closing blocks do not include `TYPE`

### 6.6 `#...` Also Participates in Matching

If the name part differs, the opening and closing blocks do not match.

## 7. Why `#...` Is Allowed in Names

The `#...` suffix exists to make boundaries more specific while keeping the main name intact.

It helps when the same name appears inside nested PML content and a plain close name would otherwise collide.

## 8. No Escape Syntax

PML does not define any escape character that disables control lines.

The reason is simple: PML is already meant to compose many formats. Adding a second escape layer would create more boundary problems than it solves.

So PML chooses:

1. no body escaping
2. no body escaping syntax
3. resolve collisions by adjusting names or adding `#...`

## 9. Self-Nesting

PML can carry another PML document, but the current layer does not recursively parse it.

At the outer layer, the inner document is just body text.
If the caller wants, it can be parsed again as PML.

## 10. Strictness and Errors

PML has strict mode only.

These cases should error:

1. invalid opening headers such as `[A key="value"]`
2. invalid closing headers such as `[/A extra]`
3. stray closing blocks such as `[/A]`
4. closing blocks with type such as `[A:yaml] ... [/A:yaml]`
5. name mismatch such as `[A#x] ... [/A#y]`

## 11. Recommended Writing Style

1. Ordinary body text may use implicit boundaries.
2. Use explicit boundaries when the body may contain standalone bracket lines or PML-like lines.
3. The opening header structure is `[NAME:TYPE]`, where `:TYPE` may be omitted.
4. Write `:TYPE` when LLMs or tools need to recognize the body format.
5. Use `#...` on the outer name when boundaries may conflict.
6. Put data in the body; its format may be `markdown`, `yaml`, `json`, or a custom type.

## 12. Reference Parse Flow

1. Scan the document line by line.
2. Recognize legal opening headers.
3. Normalize the type: fill in an empty string when omitted, lowercase it, and map `md` to `markdown`.
4. Determine the body end boundary: matching close, next legal opening header, or EOF.
5. Take the body lines and exclude the separator newline before the boundary.
6. If a name-matching close is found, use an explicit boundary; otherwise use an implicit boundary.

## 13. Non-Goals

PML does not try to provide:

1. arbitrary attributes
2. automatic object projection
3. automatic array projection
4. a one-pass full-tree builder in the core format
5. automatic interpretation of YAML, JSON, Markdown, or nested PML bodies
6. a replacement for Markdown, JSON, or YAML

## 14. Summary

PML can be summarized in eight rules:

1. documents are ordered block sequences
2. the opening header structure is `[NAME:TYPE]`, where `:TYPE` may be omitted
3. closing headers are `[/NAME]` and do not include type
4. names may include segments and `#...`
5. parsing is one layer at a time
6. bodies are returned as raw strings first
7. the separator newline before the boundary is not part of the body
8. PML is strict and does not define body escaping

PML is suited for prompt templates, structured text envelopes, intermediate payloads, and text composition.

## 15. Quick Prompts for LLMs

### 15.1 Short Version

```text
You need to understand a lightweight block format called PML.

Rules:
1. The document is an ordered sequence of blocks.
2. The opening header structure is [NAME:TYPE], where :TYPE may be omitted.
3. Optional closing headers are [/NAME].
4. If a matching close exists, the body is everything between the opening and closing headers.
5. If no matching close exists, the body ends at the next opening header or EOF.
6. TYPE may be any hint such as json or yaml; the body is preserved as raw text first.

Summary:
Treat PML as an ordered block list.
Each block contains:
- name
- type
- content
```

### 15.2 Full Version

```text
You need to understand a lightweight block format called PML.
PML outputs an ordered block list, not a tree.
PML can carry another PML document inside a body, but that is a separate layer of interpretation.

Syntax:
1. The opening header structure is [NAME:TYPE], where :TYPE may be omitted.
2. Optional closing headers are [/NAME].
3. NAME may include dot segments and a #... suffix, e.g. A.B, A.1, A#outer.
4. Type matching is case-insensitive; md is normalized to markdown.
5. The body is preserved as raw text first, and callers may interpret it by TYPE later.

Parsing:
1. Control lines must stand alone on their own lines.
2. If a matching close exists, use an explicit boundary and consume the whole middle section.
3. If no matching close exists, use an implicit boundary until the next legal opening header or EOF.
4. Closing NAME must match exactly, including segments and # suffix.

About #:
1. You can treat #... as part of the name.
2. It is not an escape syntax and not a structural field.
3. It exists to make boundaries more specific while preserving the main name.

Summary:
Treat PML as an ordered block list.
Each block contains:
- name
- type
- content
```

## 16. Optional Data Models

PML is not meant to replace JSON or YAML, but it can be mapped into data structures that are easier for software to handle.

This is an optional interpretation layer built on top of block names and body content.

Two models are recommended:

1. block sequence model
2. path tree model

### 16.1 Block Sequence Model

The block sequence model is closest to the core parser.

Its properties:

1. preserves original order
2. does not project names into a tree automatically
3. is closest to the runtime parse result
4. is easiest to keep lossless or near-lossless

A recommended shape is:

```json
{
  "blocks": [
    {
      "name": "SYSTEM",
      "type": "",
      "content": "You are a rigorous assistant."
    },
    {
      "name": "CONFIG",
      "type": "yaml",
      "content": "lang: en\ntone: concise"
    }
  ]
}
```

### 16.2 Path Tree Model

The path tree model is for path-oriented access.

For example, `A.B` can be projected as the object path `A -> B`.

The goal is not to mechanically preserve every intermediate branch, but:

1. to give a natural object-access experience when the document is clean and unambiguous
2. to fall back to arrays only when repetition or ambiguity really happens

Default behavior:

1. if a path can be uniquely assigned to an existing parent node, merge into it
2. if the required parent does not exist, create an implicit parent
3. if the same key repeats under the same parent, turn that key into an array
4. if the parent is no longer unique, stop forcing a merge and form a new branch in original order

This means:

1. `[A][A.B]` should keep natural `A.B`
2. `[A.B][A.C]` should merge into one `A` when unambiguous
3. `[A][A]` or `[A.B][A.B]` should trigger arrayization
4. `[A][A][A.B]` should not be forced into an arbitrary existing `A`

Example:

```text
[A]
root

[A.B]
child
```

```json
{
  "A": {
    "type": "",
    "content": "root",
    "order": 0,
    "B": {
      "type": "",
      "content": "child",
      "order": 1
    }
  }
}
```

#### Arrayization

Same parent + same key repeated means array.

#### Parent Ambiguity

If the parent is not unique, later branches should be kept in order instead of being guessed into one existing object.

### 16.3 Meta Field Prefix

The path tree model needs metadata fields such as:

1. `type`
2. `content`
3. `tag`
4. `order`

By default, examples may use the unprefixed form for readability.

If the caller wants to avoid collisions, the implementation may prefix these meta fields.
The recommended prefix is `$`.

If a prefix is used, conversion back from the path tree model should use the same prefix to identify meta fields.

If no prefix is used and a meta field collides with a user field, that should be an error.

### 16.4 Numeric Segments

Numeric path segments are not special.

For example:

```text
[A.1]
x

[A.2]
y
```

can be interpreted as:

```json
{
  "A": [
    {
      "1": {
        "type": "",
        "content": "x",
        "order": 0
      }
    },
    {
      "2": {
        "type": "",
        "content": "y",
        "order": 1
      }
    }
  ]
}
```

`1` and `2` are still plain path segments.
PML does not treat them as array indexes.

### 16.5 Relationship Between the Two Models

1. The block sequence model is closer to the core parser and is best for preservation, storage, conversion, and debugging.
2. The path tree model is closer to a call-oriented view and is best for clean documents with path access.
3. The path tree model prefers natural object experience when there is no repetition or ambiguity.
4. When repetition or ambiguity appears, the path tree model preserves more information through arrayization, so it is usually near-lossless but not always compact.

In short, PML core provides stable block boundaries, a name system, and body preservation.
Whether those names are projected into a block sequence model, a path tree model, or something else is an optional interpretation layer above the core format.
