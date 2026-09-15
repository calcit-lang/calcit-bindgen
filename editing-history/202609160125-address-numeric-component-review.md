# 处理明确数值 Component review / Address explicit numeric Component review

## 中文

- 修复 README 英文句子的重复冠词。
- Component compatibility 将新增的 required host import，以及 import 从 unsupported 变为 supported，判定为 breaking；新增 export 仍为 additive。
- jco/Node 对每个不安全 64 位整数分别验证 direct export 与 host-import 路径都会 trap。

## English

- Fix the duplicated article in the English README sentence.
- Classify newly required host imports and imports changing from unsupported to supported as breaking Component changes, while keeping added exports additive.
- Exercise each unsafe 64-bit integer independently through both the direct-export and host-import jco/Node paths.
