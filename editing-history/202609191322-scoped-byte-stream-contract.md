# Scoped byte-stream contract

- Accept Component Interface IR v4 while retaining v2/v3 reads.
- Render the closed `readable-byte-stream` kind as WIT `stream<u8>`.
- Reject the capability outside a direct async Component export parameter and keep non-Component generators fail closed.
