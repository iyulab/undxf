# Golden cases

Copies of what the `uncad-model` repository's golden writer produces, one synthetic drawing and its expected model per case:

| File | What it is |
|---|---|
| `<case>.dxf` | The synthetic drawing the writer produced from the case's spec |
| `<case>.expected.json` | The model a reader must produce from that drawing; the spec is the oracle |

Cases here: `g1` (a general part with a title block), `g2` (blocks nested three deep), `g6` (two coincident lines), `g7` (a title block of loose texts), `g8` (a title block in Korean under code page 949 -- read from bytes), `g9` (the same title block twice). The tests read each DXF with this crate and compare the result to the expected model exactly. Not here: `g10` (a reference to a block that does not exist -- this crate keeps the name as an unresolved reference where the expected model says the reference is absent; which is right is an open question of the model).

The files are generated, not hand-written. To regenerate after a change to the writer or the spec, from a checkout of `uncad-model`:

```
cargo run -p uncad-model-golden --example write_case -- <case> <case>.dxf <case>.expected.json
```

and copy both files here. A tree that carries both repositories side by side checks that the copies have not drifted from the writer.
