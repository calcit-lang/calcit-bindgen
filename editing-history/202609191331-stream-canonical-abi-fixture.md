# Stream Canonical ABI fixture

- Package a Component v4 export with a direct `stream<u8>` parameter.
- Lock the canonical async stream read, cancel-read, drop-readable, and task-return import names used by generated core modules.
- Execute the packaged Component with a host `StreamReader<u8>` and assert that dropping the readable end leaves Wasmtime's concurrent state empty.
