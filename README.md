# PML

> A lightweight block format for LLM workflows, prompt engineering, and structured text composition

[中文说明](./README_CN.md) | [English Spec](./PML-format-spec.md) | [中文规范](./PML-format-spec_CN.md) | [Grammar (EBNF)](./PML-grammar.ebnf.md)

PML can formally be read as **Prompt Meta Language**.

It also keeps some intentionally open-ended interpretations, such as:

- **Peer as a Markup Language**
- **Patchwork Mosaic Language**
- **Prompt Modular Language**
- **PML forMat Language**

That ambiguity is part of the design.

PML is a block-oriented text format for packaging heterogeneous content into one stable document.
It does not replace JSON, YAML, or Markdown. It gives them a clear outer boundary.
It makes a document readable for humans and stable for programs and LLM systems to process.

PML is useful when one file needs to carry things like:

- system prompts
- user input
- config fragments
- JSON, YAML, Markdown, plain text, and more
- even another PML document

Another useful property is that a README or config file written in PML can also serve directly as a configuration file with extensive inline annotations.

> PML is not just a "data format". It is also a format for organizing prompt-ready, layered, and mixed text content, including PML itself.

## What It Solves

Markdown plus fenced blocks is simple, but nested composition and long-term boundary stability are often fragile.
PML takes a narrower approach:

- split content into explicit named blocks
- attach an optional body type
- preserve the body as raw text
- delay deeper interpretation

## Three Representations

- Original format:
  the `.pml` source text itself
- Block sequence model:
  the ordered parse result and the current intermediate model
- Path tree model:
  a higher-level projection for path-oriented access

## Minimal Example

```text
[SYSTEM]
You are a rigorous assistant.

[CONFIG:yaml]
lang: en
tone: concise
```

Block sequence model:

```json
[
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
```

Path tree model:

```json
{
  "SYSTEM": {
    "type": "",
    "content": "You are a rigorous assistant.",
    "order": 0
  },
  "CONFIG": {
    "type": "yaml",
    "content": "lang: en\ntone: concise",
    "order": 1
  }
}
```

## Example Snippets

These are simple single-file, dependency-free implementations for convenient use.
They are not intended as the best implementation or the highest-performance version.

### Rust

```rust
mod pml;

use pml::{parse_pml, PmlBuilder, PmlTreeOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "[SYSTEM]\nYou are a rigorous assistant.\n\n[CONFIG:yaml]\nlang: en\ntone: concise\n";
    let blocks = parse_pml(source)?;
    let tree = pml::blocks_to_tree(&blocks, &PmlTreeOptions::default())?;

    let mut builder = PmlBuilder::new();
    builder
        .push_paired("SYSTEM", None, "You are a rigorous assistant.")?
        .push_paired("CONFIG", Some("yaml"), "lang: en\ntone: concise")?;

    println!("{tree:#?}");
    println!("{}", builder.build());
    Ok(())
}
```

### Python

```python
import pml

source = "[SYSTEM]\nYou are a rigorous assistant.\n"
blocks = pml.parse_pml(source)
tree = pml.parse_pml_tree(source)

print(blocks[0].name)
print(tree["SYSTEM"]["$content"])
```

### Node.js

```js
const pml = require("./pml.js");

const source = "[SYSTEM]\nYou are a rigorous assistant.\n";
const blocks = pml.parsePml(source);
const tree = pml.parsePmlTree(source);

console.log(blocks[0].name);
console.log(tree.SYSTEM.$content);
```

### Java

```java
import java.util.List;
import java.util.Map;

String source = "[SYSTEM]\nYou are a rigorous assistant.\n";
List<Pml.PmlBlock> blocks = Pml.parsePml(source);
Map<String, Object> tree = Pml.parsePmlTree(source, new Pml.PmlTreeOptions());

System.out.println(blocks.get(0).name);
System.out.println(((Map<String, Object>) tree.get("SYSTEM")).get("$content"));
```

## Notes

- Ordinary body text may use implicit boundaries.
- Use explicit boundaries when the body may contain standalone bracket lines or legal PML control lines.
- Use `#...` on outer names when boundaries may conflict.
- The full grammar and model rules are in the spec files.

## License

This project is licensed under the [MIT License](./LICENSE).
