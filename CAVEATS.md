# Caveats — Lessons from the Pit

## parking_lot::Mutex deadlock rule

NEVER hold a lock across a call that may re-lock the same mutex.

```rust
// DEADLOCK — temp guard lives through body
if expr.lock().foo() {            // lock acquired, guard alive for `if` body
    let x = expr.lock().bar();    // second lock → deadlock
}
if let Some(v) = expr.lock().foo() {
    expr.lock().bar();            // same → deadlock
}
```

Fix: extract value before the conditional:
```rust
let v = expr.lock().foo();
if v {
    let x = expr.lock().bar();    // safe — first lock released at `;`
}
```

Same applies to `if let` / `while let` patterns.

## UTF-8 slicing rule

NEVER use byte positions from `str::len()` for indexing into `&str`.

```rust
// PANIC — `len()` returns bytes, CJK chars are 3 bytes each
let block_len = t.len();
// ...
&text[pos..pos + chunk];   // byte index lands in mid-char → panic
```

Fix: keep `char_index` and `byte_offset` as separate concepts:

| Type | Purpose |
|------|---------|
| `t.chars().count()` | character-level counting / page splitting |
| `str::char_indices()` | convert char_index → byte_offset for slicing |
| `&s[byte_start..byte_end]` | the only way to slice; both ends must be on char boundaries |

```rust
let block_len = t.chars().count();   // char count
// ...
let chars: Vec<(usize, usize)> = text.char_indices()
    .map(|(i, c)| (i, c.len_utf8()))
    .collect();
let start_byte = chars[char_start].0;
let end_byte = if char_end < chars.len() {
    chars[char_end].0
} else {
    text.len()
};
&text[start_byte..end_byte]           // safe slice
```

Never mix the two — not even in variable naming. `char_range` should store char indices,
`byte_range` should store byte offsets. One panic is all it takes.
