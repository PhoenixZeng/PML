# PML

[English README](./README.md) | [中文规范](./PML-format-spec_CN.md) | [English Spec](./PML-format-spec.md)

> 面向 LLM、提示词工程与结构化文本拼接的轻量块格式

作为正式名称，PML 可以理解为 **Prompt Meta Language**。

同时，它也保留了一点有意为之的开放语义，例如：
- **Peer as a Markup Language**
- **Patchwork Mosaic Language**
- **Prompt Modular Language**
- **PML forMat Language**

这种模糊不是缺陷，而是设计的一部分。

PML 是一种把异构文本稳定装进同一份文档的块式格式。
它不取代 JSON、YAML 或 Markdown，而是给这些内容提供清晰的外层边界。
让一份文档既适合人类读，也适合程序和 LLM AI 稳定处理。

它适合一份文件里同时承载这些内容：

- system prompt
- user input
- 配置片段
- JSON、YAML、Markdown、纯文本等等
- 甚至是另一个 PML 格式

另一个作用是如果你使用 PML 来编写README或配置文件，它甚至可以直接充当具有大量注释说明的配置文件

> PML 不只是一个“数据格式”，更是一种用来组织可提示、可分层、可混合文本内容的格式(包括混合 PML 自身)。

## 它解决什么问题

Markdown 加代码块很方便，但在自嵌套、长期拼接和边界稳定性上并不总是可靠。
PML 的做法更克制：

- 把内容切成显式命名块
- 给正文附一个可选类型
- 正文先按原始字符串保留
- 更深层的解释延后处理

## 三种表示

- 原始格式：
  也就是 `.pml` 文本本身
- 块序模型：
  最贴近解析结果，也是当前实现里的中间态
- 路径树模型：
  更适合按路径读取的高层投影

## 最小例子

```text
[SYSTEM]
你是一个严谨助手。

[CONFIG:yaml]
lang: zh-CN
tone: concise
```

块序模型：

```json
[
  {
    "name": "SYSTEM",
    "type": "text",
    "content": "你是一个严谨助手。"
  },
  {
    "name": "CONFIG",
    "type": "yaml",
    "content": "lang: zh-CN\ntone: concise"
  }
]
```

路径树模型：

```json
{
  "SYSTEM": {
    "type": "text",
    "content": "你是一个严谨助手。",
    "order": 0
  },
  "CONFIG": {
    "type": "yaml",
    "content": "lang: zh-CN\ntone: concise",
    "order": 1
  }
}
```

## 例子片段

这里只是单文件无依赖的简单实现，方便简单使用，并非最佳实现和性能。

### Rust

```rust
mod pml;

use pml::{parse_pml, PmlBuilder, PmlTreeOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "[SYSTEM]\n你是一个严谨助手。\n\n[CONFIG:yaml]\nlang: zh-CN\ntone: concise\n";
    let blocks = parse_pml(source)?;
    let tree = pml::blocks_to_tree(&blocks, &PmlTreeOptions::default())?;

    let mut builder = PmlBuilder::new();
    builder
        .push_paired("SYSTEM", None, "你是一个严谨助手。")?
        .push_paired("CONFIG", Some("yaml"), "lang: zh-CN\ntone: concise")?;

    println!("{tree:#?}");
    println!("{}", builder.build());
    Ok(())
}
```

### Python

```python
import pml

source = "[SYSTEM]\n你是一个严谨助手。\n"
blocks = pml.parse_pml(source)
tree = pml.parse_pml_tree(source)

print(blocks[0].name)
print(tree["SYSTEM"]["$content"])
```

### Node.js

```js
const pml = require("./pml.js");

const source = "[SYSTEM]\n你是一个严谨助手。\n";
const blocks = pml.parsePml(source);
const tree = pml.parsePmlTree(source);

console.log(blocks[0].name);
console.log(tree.SYSTEM.$content);
```

### Java

```java
import java.util.List;
import java.util.Map;

String source = "[SYSTEM]\n你是一个严谨助手。\n";
List<Pml.PmlBlock> blocks = Pml.parsePml(source);
Map<String, Object> tree = Pml.parsePmlTree(source, new Pml.PmlTreeOptions());

System.out.println(blocks.get(0).name);
System.out.println(((Map<String, Object>) tree.get("SYSTEM")).get("$content"));
```

## 说明

- 普通正文优先用短块。
- 当正文里可能出现合法 PML 控制行时，优先用配对块。
- 当边界可能冲突时，优先给块名加 `#...`。
- 详细语法和模型规则见规范文件。

## 许可证

本项目采用 [MIT License](./LICENSE)。
