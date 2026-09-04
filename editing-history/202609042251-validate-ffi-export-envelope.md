# Validate the full FFI export envelope / 校验完整 FFI export envelope

- Model the versioned `ffi.export` envelope fields that were previously ignored
  while only the embedded Interface IR document was deserialized.
- Require the published Interface IR v2 schema ID, dependency exclusion,
  accurate summary counts, complete diagnostics, and the core revision digest.
- Keep raw Interface IR v2 documents supported for existing generated fixtures
  and downstream tools.
- Reject unknown v1 envelope fields so schema evolution requires an explicit
  consumer update instead of being silently accepted.
