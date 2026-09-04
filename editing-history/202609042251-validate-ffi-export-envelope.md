# Validate the full FFI export envelope / 校验完整 FFI export envelope

- Model the versioned `ffi.export` envelope fields that were previously ignored
  while only the embedded Interface IR document was deserialized. Keep this
  strict wire model private so the previously public minimal `Envelope` API
  remains source-compatible for downstream crates.
- Require the published Interface IR v2 schema ID, dependency exclusion,
  accurate summary counts, complete diagnostics, and the core revision digest.
- Keep raw Interface IR v2 documents supported for existing generated fixtures
  and downstream tools.
- Reject unknown v1 envelope fields so schema evolution requires an explicit
  consumer update instead of being silently accepted. Apply the same
  fail-closed rule to raw v2 documents and every nested structured IR node, in
  line with the published schema's `additionalProperties: false` contract.
- Cover both a complete non-empty diagnostic that participates in summary and
  revision validation, and a diagnostic missing a required field.
- Cross-check every structured diagnostic definition/code pair against the
  embedded definition inventory, and require every advertised diagnostic code
  to have structured evidence.
- Clarify that namespace is a required nullable v1 filter field and that loader
  output is the embedded document after envelope metadata validation.
