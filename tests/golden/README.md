# Golden cases

Copies of what the `uncad-model` repository's golden writer produces, one synthetic drawing and its expected model per case:

| File | What it is |
|---|---|
| `<case>.dxf` | The synthetic drawing the writer produced from the case's spec |
| `<case>.expected.json` | The model a reader must produce from that drawing; the spec is the oracle |

Cases here: `g1` (a general part with a title block), `g2` (blocks nested three deep), `g5` (dense dimensions), `g6` (two coincident lines), `g7` (a title block of loose texts), `g8` (a title block in Korean under code page 949 -- read from bytes), `g9` (the same title block twice), `g11` (a mirrored part), `g12` (bulged and wide polylines), `g13` (justified text), `g14` (a sheet of viewports), `g15` (a polygon mesh), `g16` (ordinate dimensions), `g17` (hatches bounded by edge paths) `g18` (lines stating their own linetype, scale and weight) `g19` (the sheet of `g14` as a layout that is not the current one writes it: every viewport numbered 0) and `g20` (multileaders whose line type each layer of the format settles -- the style, the entity's override, a missing style, the lines' own -- an R2010 file) and `g21` (the ways a drawing states a tolerance -- stacked limit deviations in either order and sign, a value typed over the measurement, a fit symbol, a general tolerance in the title block -- each read as written) and `g22` (a sheet of three views inside a frame: a closed rectangle the size of an A3 drawing space, a plate with two holes seen from the front, the top and the side, and a line past the frame), `g23` (a title block of loose texts laid out as ISO 7200 draws its fields: each name a small text at the top left of its field, its value a larger text beneath it -- one value over two lines, two lines of one height, and a centred value among them), `g24` (one view whose outline is separate lines and an arc meeting at their ends -- a plate with one rounded corner, a hole and a line that touches nothing). The tests read each DXF with this crate and compare the result to the expected model -- most of them exactly, the rest (`g1`, `g5`, `g8`) through the properties their own tests name. Not here: `g10` (a reference to a block that does not exist -- this crate keeps the name as an unresolved reference).

The files are generated, not hand-written. To regenerate after a change to the writer or the spec, from a checkout of `uncad-model`:

```
cargo run -p uncad-model-golden --example write_case -- <case> <case>.dxf <case>.expected.json
```

and copy both files here. A tree that carries both repositories side by side checks that the copies have not drifted from the writer.
