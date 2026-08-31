# PR #11 lifecycle review follow-up / 生命周期 review 跟进

- Preserve canonical v2 Interface IR serialization: absent v3 `stream` and
  `resource` fields are omitted instead of encoded as `null`.
- Validate lifecycle references against declared `Parameter.position` values,
  rather than assuming parameter positions are contiguous array indices.
- Document `lowering.stream` and `lowering.resource` as breaking compatibility
  contract fields, and add regression coverage for all three rules.

验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
`cargo test`、`cargo package --allow-dirty`。
