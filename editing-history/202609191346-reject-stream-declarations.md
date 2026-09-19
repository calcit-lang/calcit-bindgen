# Reject streams in Component declarations

- Reject `ReadableByteStream` recursively in Component Struct fields and Enum payloads.
- Cover named Struct and Enum references in both parameter and result positions so monomorphic declarations cannot bypass the direct async-export parameter rule.
