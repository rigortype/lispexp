# Round-trip target: verbatim (source-slice) is guaranteed; constructive is a future best-effort

This clarifies ADR-0002's "keep round-trip feasible" against the zero-copy design (ADR-0008). Because `Parsed` retains the source and every Datum carries a byte span, **verbatim** round-trip of an unmodified tree is free and lossless — reproduce any region by slicing the source, including comments and original whitespace, which is why comments are deliberately not stored as tree nodes at all. **Constructive** serialization — re-emitting a modified or synthesized tree without the original source — is an explicit non-guarantee for now, but the data model is shaped to keep it feasible: the `Notation` tag (ADR-0002) is what lets a future constructive serializer choose between `'x` and `(quote x)`, and it is retained precisely for that future, not for verbatim round-trip (which needs only the source). The known limitation of constructive serialization is that comments, not being in the tree, would be lost; that is acceptable given comments are out of `cccc`'s scope.

> **Amended 2026-07-02:** two constructive losses named above are now fixed.
> `DatumKind::Prefixed` gained an `arg: Option<Box<Datum>>` field carrying the
> prefix's auxiliary datum — the metadata form for `Meta` (`^meta target`) and
> the feature test for `FeatureConditional` (`#+sbcl form`) — both of which
> the reader previously parsed and discarded. A constructive serializer can
> now re-emit `^meta target` or `#+sbcl form` in full; before this change it
> could only recover `target`/`form`, having silently lost the annotation or
> feature test. The data model's remaining constructive losses are exactly:
>
> - Comments and whitespace, which live in the lexer layer, not the tree, by
>   the design above (unchanged).
> - `#;`/`#_` discarded data: the reader still reads and drops the discarded
>   datum itself (only the target that follows is kept), so a constructive
>   serializer cannot reproduce what was discarded.
>
> No other tree-shape change in this refinement affects constructive
> feasibility.
