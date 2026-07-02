//! Code-vs-data walker (ADR-0026).
//!
//! A best-effort *pruning visitor* over a [`Datum`] tree that classifies each
//! node as [`Class::Code`] or [`Class::Data`] and lets a consumer prune whole
//! subtrees (the search need: descend into code, skip quoted data). It is a
//! utility layer over the reader's tree — it interprets structure already
//! present and evaluates nothing (ADR-0001).
//!
//! # Pruning safely: sealed vs. porous data
//!
//! The binary [`Class`] hides a distinction that matters the moment a consumer
//! actually *prunes*: not all `Data` can be skipped without losing code.
//!
//! - A hard `quote` (or a hash literal, or discarded content) is **sealed** —
//!   nothing inside can ever be code, so returning [`Walk::Skip`] on it is a
//!   safe optimization.
//! - A **quasiquote template** is `Data` too, but it is **porous**: a matching
//!   nested `unquote` flips its *contents* back to code (`` `(a ,(f x)) `` — the
//!   `(f x)` is code). `Skip` here silently drops that code.
//!
//! So the tempting idiom `if class == Class::Data { Walk::Skip }` is a
//! **footgun** — it under-counts every unquoted form inside a quasiquote. Two
//! ways to stay correct:
//!
//! - Prune with [`walk_regions`], which reports a three-way [`Region`]:
//!   [`Region::is_prunable`] is `true` only for [`Region::SealedData`], so you
//!   `Skip` sealed data and `Descend` through porous templates.
//! - Or, with the binary [`walk`], only ever `Skip` a node you have *handled
//!   yourself*; let every unhandled node `Descend` and trust the walker's own
//!   depth tracking to reach nested code. Never use `Skip` as a blanket
//!   data-pruning shortcut.
//!
//! The classification criterion is uniformly "can this be evaluated?", driven
//! by reader-macro nesting:
//!
//! - `Quote` opens an **absolute** data region (deep): everything inside is
//!   `Data` and unquote cannot escape it.
//! - `Quasiquote` opens a data region tracked by a **depth counter** (`+1`);
//!   `Unquote`/`UnquoteSplicing` step back toward code (`-1`, clamped at 0). A
//!   node is `Code` iff the quasiquote depth is 0 *and* it is not under a hard
//!   `Quote`. This classifies double-unquote (`` `` `,,c `` ``) as `Code`, which
//!   a boolean flag could not.
//! - `VarQuote`/`FunctionQuote` (`#'foo`), `Deref` (`@x`), `Splice`, and
//!   `HashFn` (`#(...)`) are **context-transparent**: at top level they are code
//!   references, but inside a quasiquote template or a quote they are data like
//!   their surroundings (`` `(f @x) `` — `x` is template data).
//! - `ReadEval` (`#.x`) marks its contents as `Code` **unconditionally** — `#.`
//!   is evaluated at *read* time, even inside `quote`.
//! - `HashLiteral` (`#(1 2 3)`, `#u8(...)`, tagged `#inst …`), `LabelRef`
//!   (`#n#`), and `Discard` are `Data`.
//! - `Meta`, `Mutable`, and `Label` are **context-transparent** (inherit the
//!   parent's class); a `FeatureConditional`/`ReaderConditional`'s guarded form
//!   is transparent too. A `Prefixed`'s auxiliary `arg` (metadata form /
//!   feature test) is visited as `Data`.
//!
//! The visitor is primary because *arbitrary* per-node pruning cannot be
//! expressed by a bare iterator. [`code_nodes`] layers the one common fixed
//! policy — prune sealed data, descend porous templates, yield only code — on
//! top as a pre-order [`Iterator`], for consumers that just want "every code
//! node" and the combinators (`filter`/`find`/`take`) that come with it.

use std::ops::ControlFlow;

use crate::datum::{Datum, DatumKind, Prefix};

/// Whether a subtree is executable code or inert data (ADR-0026).
///
/// This is the *binary* view. It is enough to classify a node, but **not** to
/// decide whether skipping it is safe: a quasiquote template is `Data` yet can
/// carry nested code (see [`Region`]). To prune, prefer [`walk_regions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Can be evaluated — descend into it for code analysis.
    Code,
    /// Inert data — a search prunes it.
    Data,
}

/// A refinement of [`Class`] that splits `Data` by whether it is safe to prune
/// (ADR-0026).
///
/// [`walk`]'s binary `Data` collapses two cases a *pruning* consumer must keep
/// apart. `walk_regions` reports this three-way instead so a [`Walk::Skip`]
/// never silently drops code:
///
/// - [`SealedData`](Region::SealedData) — a hard `quote`, a hash literal, or
///   discarded content: nothing inside can become code. Safe to `Skip`.
/// - [`PorousData`](Region::PorousData) — a quasiquote template (depth > 0):
///   inert *here*, but a matching nested `unquote` re-enters code, so its
///   children must be walked. `Skip` would drop that code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Region {
    /// Can be evaluated — descend into it for code analysis.
    Code,
    /// Data that can never contain code (hard `quote`, hash literal, discard).
    /// Safe to prune with [`Walk::Skip`].
    SealedData,
    /// A quasiquote template: data at this depth, but a nested `unquote` can
    /// flip its contents back to code. Must be descended into, not pruned.
    PorousData,
}

impl Region {
    /// The binary [`Class`] for this region: everything but [`Region::Code`] is
    /// [`Class::Data`].
    pub fn class(self) -> Class {
        match self {
            Region::Code => Class::Code,
            Region::SealedData | Region::PorousData => Class::Data,
        }
    }

    /// Whether returning [`Walk::Skip`] on this node is a safe optimization —
    /// `true` only for [`Region::SealedData`]. Skipping code or a porous
    /// quasiquote template would drop nested code.
    pub fn is_prunable(self) -> bool {
        matches!(self, Region::SealedData)
    }
}

/// What the visitor callback asks the walker to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Walk {
    /// Recurse into this datum's children.
    Descend,
    /// Do not recurse into this datum's children.
    Skip,
    /// Abort the whole walk immediately (e.g. after a first match).
    Stop,
}

/// The classification region a datum sits in: an absolute quote barrier and a
/// quasiquote depth. `Class` is `Data` iff under a hard quote or depth > 0.
#[derive(Debug, Clone, Copy)]
struct Ctx {
    hard_quote: bool,
    qq: u32,
}

impl Ctx {
    /// Top level and code regions.
    const TOP: Ctx = Ctx {
        hard_quote: false,
        qq: 0,
    };
    /// An absolute data region.
    const DATA: Ctx = Ctx {
        hard_quote: true,
        qq: 0,
    };

    /// The region this context establishes. A hard quote seals; a positive
    /// quasiquote depth is porous; otherwise code.
    fn region(self) -> Region {
        if self.hard_quote {
            Region::SealedData
        } else if self.qq > 0 {
            Region::PorousData
        } else {
            Region::Code
        }
    }
}

/// The region a prefix's inner datum sits in, given the region of the prefix.
fn inner_ctx(prefix: Prefix, ctx: Ctx) -> Ctx {
    match prefix {
        // Quote is an absolute, deep data barrier that unquote cannot escape.
        Prefix::Quote => Ctx::DATA,
        // Quasiquote deepens the data region; unquotes step back toward code.
        Prefix::Quasiquote => Ctx {
            qq: ctx.qq + 1,
            ..ctx
        },
        Prefix::Unquote | Prefix::UnquoteSplicing => Ctx {
            qq: ctx.qq.saturating_sub(1),
            ..ctx
        },
        // Code references are context-transparent: code at top level, but data
        // inside a quasiquote template or quote (`` `(f @x) `` — `x` is data).
        Prefix::VarQuote
        | Prefix::FunctionQuote
        | Prefix::Deref
        | Prefix::Splice
        | Prefix::HashFn => ctx,
        // `#.` is evaluated at *read* time — code even inside `quote`.
        Prefix::ReadEval => Ctx::TOP,
        // Context-transparent wrappers. A feature/reader conditional's guarded
        // form and a metadata target inherit the surrounding class.
        Prefix::Meta
        | Prefix::Mutable
        | Prefix::FeatureConditional { .. }
        | Prefix::ReaderConditional { .. } => ctx,
        // Discarded content is inert. (The reader consumes `#_`/`#;` and never
        // emits a Discard-prefixed datum; kept for manually built trees.)
        Prefix::Discard => Ctx::DATA,
    }
}

/// The region a datum occupies. `HashLiteral` and `LabelRef` are sealed data
/// regardless of surrounding region.
fn node_region(datum: &Datum<'_>, ctx: Ctx) -> Region {
    match &datum.kind {
        DatumKind::HashLiteral { .. } | DatumKind::LabelRef { .. } => Region::SealedData,
        _ => ctx.region(),
    }
}

/// Walk each top-level datum in `data`, invoking `visit(datum, class)` in
/// pre-order. When the callback returns [`Walk::Skip`], that datum's children
/// are pruned; [`Walk::Descend`] recurses; [`Walk::Stop`] aborts the whole
/// walk. Top-level data start as [`Class::Code`].
///
/// **Do not use `Skip` to prune on [`Class::Data`].** A quasiquote template is
/// `Data`, yet a nested `unquote` inside it is code; pruning on `Data` drops
/// that code (see the module docs on *sealed vs. porous* data). This example
/// therefore descends through everything and only *counts* code — to prune
/// safely, reach for [`walk_regions`] and [`Region::is_prunable`].
///
/// ```
/// use lispexp::{parse, walk, Class, Walk, DatumKind, Options};
///
/// // A quasiquote whose unquoted `(f y)` is real code, next to quoted data.
/// let parsed = parse("`(a ,(f y) '(b c))", &Options::scheme());
/// let mut code_lists = 0;
/// walk(&parsed.data, |datum, class| {
///     if class == Class::Code && matches!(datum.kind, DatumKind::List { .. }) {
///         code_lists += 1;
///     }
///     Walk::Descend // never blanket-Skip on Data — it would drop `(f y)`
/// });
/// assert_eq!(code_lists, 1); // the unquoted `(f y)`, not the quoted `(b c)`
/// ```
pub fn walk<'a, 't, F>(data: &'a [Datum<'t>], mut visit: F)
where
    F: FnMut(&'a Datum<'t>, Class) -> Walk,
{
    walk_regions(data, |datum, region| visit(datum, region.class()));
}

/// Like [`walk`], but the callback receives a three-way [`Region`] instead of
/// the binary [`Class`], so pruning is safe: [`Region::is_prunable`] tells you
/// whether [`Walk::Skip`] would lose code.
///
/// ```
/// use lispexp::{parse, walk_regions, Walk, DatumKind, Options};
///
/// // Prune sealed data, but still descend into the porous quasiquote template.
/// let parsed = parse("`(a ,(f y) '(b c))", &Options::scheme());
/// let mut code_lists = 0;
/// let mut pruned = 0;
/// walk_regions(&parsed.data, |datum, region| {
///     if region.is_prunable() {
///         pruned += 1;
///         return Walk::Skip; // safe: '(b c) can never contain code
///     }
///     if region == lispexp::Region::Code && matches!(datum.kind, DatumKind::List { .. }) {
///         code_lists += 1;
///     }
///     Walk::Descend
/// });
/// assert_eq!(code_lists, 1); // `(f y)` is still reached inside the template
/// assert_eq!(pruned, 1); // the quoted `'(b c)` subtree was skipped whole
/// ```
pub fn walk_regions<'a, 't, F>(data: &'a [Datum<'t>], mut visit: F)
where
    F: FnMut(&'a Datum<'t>, Region) -> Walk,
{
    for datum in data {
        if walk_datum(datum, Ctx::TOP, &mut visit).is_break() {
            return;
        }
    }
}

fn walk_datum<'a, 't, F>(datum: &'a Datum<'t>, ctx: Ctx, visit: &mut F) -> ControlFlow<()>
where
    F: FnMut(&'a Datum<'t>, Region) -> Walk,
{
    match visit(datum, node_region(datum, ctx)) {
        Walk::Skip => return ControlFlow::Continue(()),
        Walk::Stop => return ControlFlow::Break(()),
        Walk::Descend => {}
    }
    let mut flow = ControlFlow::Continue(());
    for_each_child(datum, ctx, |child, cctx| {
        if flow.is_continue() {
            flow = walk_datum(child, cctx, visit);
        }
    });
    flow
}

/// Invoke `f` on each structural child of `datum`, paired with the region
/// context it inherits, in source order. The single place that knows a datum's
/// child shape and how each prefix shifts the region — both the [`walk_regions`]
/// visitor and the [`code_nodes`] iterator route their descent through it. Takes
/// a callback rather than returning an iterator so neither hot path allocates.
fn for_each_child<'a, 't>(datum: &'a Datum<'t>, ctx: Ctx, mut f: impl FnMut(&'a Datum<'t>, Ctx)) {
    match &datum.kind {
        DatumKind::List { items, tail, .. } => {
            for item in items {
                f(item, ctx);
            }
            if let Some(tail) = tail {
                f(tail, ctx);
            }
        }
        DatumKind::Prefixed {
            prefix, inner, arg, ..
        } => {
            // The auxiliary datum (metadata form / feature test) is inert
            // metadata: visited as data before the inner form.
            if let Some(arg) = arg {
                f(arg, Ctx::DATA);
            }
            f(inner, inner_ctx(*prefix, ctx));
        }
        // A hash literal's content is data; a datum label is transparent.
        DatumKind::HashLiteral {
            inner: Some(inner), ..
        } => f(inner, Ctx::DATA),
        DatumKind::Label { inner, .. } => f(inner, ctx),
        _ => {}
    }
}

/// A pre-order iterator over the [`Class::Code`] nodes of a datum forest,
/// created by [`code_nodes`].
///
/// Unlike the [`walk`]/[`walk_regions`] visitors — which let a consumer make an
/// arbitrary prune decision per node — this fixes the common policy: prune
/// [`Region::SealedData`], descend [`Region::PorousData`] (so nested unquoted
/// code is reached), and yield each code node once, parent before child. A
/// caller that just wants "every code node" writes a `for` loop instead of a
/// callback, and gets `Iterator`'s combinators (`filter`, `find`, `take`, …)
/// with short-circuiting for free.
pub struct CodeNodes<'a, 't> {
    // A DFS worklist of (datum, region context). Children are pushed reversed so
    // the leftmost is popped next, giving pre-order over the code nodes.
    stack: Vec<(&'a Datum<'t>, Ctx)>,
}

impl<'a, 't> Iterator for CodeNodes<'a, 't> {
    type Item = &'a Datum<'t>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((datum, ctx)) = self.stack.pop() {
            // Sealed data can never contain code — prune it, don't descend.
            let region = node_region(datum, ctx);
            if region == Region::SealedData {
                continue;
            }
            let start = self.stack.len();
            for_each_child(datum, ctx, |child, cctx| self.stack.push((child, cctx)));
            self.stack[start..].reverse();
            // Code nodes are yielded; porous templates are only descended.
            if region == Region::Code {
                return Some(datum);
            }
        }
        None
    }
}

// Once the worklist drains it stays drained, so `next` never yields again after
// returning `None` — the contract `FusedIterator` lets combinators rely on.
impl std::iter::FusedIterator for CodeNodes<'_, '_> {}

/// Iterate the [`Class::Code`] nodes of `data` in pre-order — the read-only,
/// fixed-policy counterpart to [`walk`]. It always prunes sealed data and
/// descends porous quasiquote templates, so nested unquoted code is reached but
/// quoted data is never yielded.
///
/// ```
/// use lispexp::{parse, code_nodes, DatumKind, Options};
///
/// // Only the unquoted `(f y)` is code; the quoted `(b c)` is pruned.
/// let parsed = parse("`(a ,(f y) '(b c))", &Options::scheme());
/// let code_lists = code_nodes(&parsed.data)
///     .filter(|d| matches!(d.kind, DatumKind::List { .. }))
///     .count();
/// assert_eq!(code_lists, 1);
/// ```
pub fn code_nodes<'a, 't>(data: &'a [Datum<'t>]) -> CodeNodes<'a, 't> {
    CodeNodes {
        stack: data.iter().rev().map(|d| (d, Ctx::TOP)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use crate::reader::parse;

    /// Collect `(source-text, class)` for every visited node, descending all.
    fn classes<'a>(src: &'a str, opts: &Options) -> Vec<(&'a str, Class)> {
        let parsed = parse(src, opts);
        // Leak-free: borrow the source via spans.
        let mut out = Vec::new();
        walk(&parsed.data, |d, c| {
            out.push((d.span.text(src), c));
            Walk::Descend
        });
        out
    }

    fn class_of(src: &str, opts: &Options, needle: &str) -> Class {
        classes(src, opts)
            .into_iter()
            .find(|(t, _)| *t == needle)
            .unwrap_or_else(|| panic!("{needle:?} not visited in {src:?}"))
            .1
    }

    #[test]
    fn top_level_and_list_items_are_code() {
        let s = Options::scheme();
        assert_eq!(class_of("(f x)", &s, "(f x)"), Class::Code);
        assert_eq!(class_of("(f x)", &s, "f"), Class::Code);
        assert_eq!(class_of("(f x)", &s, "x"), Class::Code);
    }

    #[test]
    fn quote_makes_inner_data_deep() {
        let s = Options::scheme();
        assert_eq!(class_of("'(a b)", &s, "'(a b)"), Class::Code); // the quote form
        assert_eq!(class_of("'(a b)", &s, "(a b)"), Class::Data);
        assert_eq!(class_of("'(a b)", &s, "a"), Class::Data);
        assert_eq!(class_of("'(a b)", &s, "b"), Class::Data);
    }

    #[test]
    fn quasiquote_unquote_flips_back() {
        let s = Options::scheme();
        // `(a ,b): a is data, b flips back to code.
        assert_eq!(class_of("`(a ,b)", &s, "a"), Class::Data);
        assert_eq!(class_of("`(a ,b)", &s, "b"), Class::Code);
    }

    #[test]
    fn double_unquote_under_double_quasiquote_is_code() {
        let s = Options::scheme();
        // ``(,,c): two quasiquotes, two unquotes -> c is back at depth 0.
        assert_eq!(class_of("``(,,c)", &s, "c"), Class::Code);
    }

    #[test]
    fn unquote_cannot_escape_hard_quote() {
        let s = Options::scheme();
        // '(,b): quote is absolute; the unquote does not reach code.
        assert_eq!(class_of("'(,b)", &s, "b"), Class::Data);
    }

    #[test]
    fn hash_literal_is_data() {
        let s = Options::scheme();
        // #(1 2 3) is a vector literal -> data even at top level.
        assert_eq!(class_of("#(1 2 3)", &s, "#(1 2 3)"), Class::Data);
    }

    #[test]
    fn function_quote_is_code() {
        let c = Options::common_lisp();
        assert_eq!(class_of("#'foo", &c, "foo"), Class::Code);
    }

    #[test]
    fn deref_is_code() {
        let c = Options::clojure();
        assert_eq!(class_of("@x", &c, "x"), Class::Code);
    }

    #[test]
    fn deref_inside_quasiquote_stays_data() {
        // `(f @x): the deref is template data at depth 1; it does not reset the
        // quasiquote depth.
        let c = Options::clojure();
        assert_eq!(class_of("`(f @x)", &c, "x"), Class::Data);
        // ...but an unquote around it flips back to code as usual.
        assert_eq!(class_of("`(f ~@y)", &c, "y"), Class::Code);
    }

    #[test]
    fn function_quote_inside_quote_stays_data() {
        let c = Options::common_lisp();
        assert_eq!(class_of("'(f #'a)", &c, "a"), Class::Data);
    }

    #[test]
    fn read_eval_is_code_even_under_quote() {
        // #. runs at read time — quote cannot inert it.
        let c = Options::common_lisp();
        assert_eq!(class_of("'(a #.(f))", &c, "(f)"), Class::Code);
    }

    #[test]
    fn stop_aborts_the_walk() {
        let s = Options::scheme();
        let src = "(a b) (c d)";
        let parsed = parse(src, &s);
        let mut visited = Vec::new();
        walk(&parsed.data, |d, _| {
            visited.push(d.span.text(src));
            if d.span.text(src) == "b" {
                Walk::Stop
            } else {
                Walk::Descend
            }
        });
        assert!(visited.contains(&"b"));
        // Nothing after the stop point is visited — not even siblings or the
        // next top-level form.
        assert!(!visited.contains(&"(c d)"));
        assert!(!visited.contains(&"c"));
    }

    /// Collect `(source-text, region)` for every visited node, descending all.
    fn regions<'a>(src: &'a str, opts: &Options) -> Vec<(&'a str, Region)> {
        let parsed = parse(src, opts);
        let mut out = Vec::new();
        walk_regions(&parsed.data, |d, r| {
            out.push((d.span.text(src), r));
            Walk::Descend
        });
        out
    }

    fn region_of(src: &str, opts: &Options, needle: &str) -> Region {
        regions(src, opts)
            .into_iter()
            .find(|(t, _)| *t == needle)
            .unwrap_or_else(|| panic!("{needle:?} not visited in {src:?}"))
            .1
    }

    #[test]
    fn hard_quote_is_sealed_quasiquote_template_is_porous() {
        let s = Options::scheme();
        // A hard quote seals: nothing inside can become code.
        assert_eq!(region_of("'(a b)", &s, "(a b)"), Region::SealedData);
        // A quasiquote template is data, but porous — an unquote can escape it.
        assert_eq!(region_of("`(a ,b)", &s, "(a ,b)"), Region::PorousData);
        // The unquoted form itself is back to code.
        assert_eq!(region_of("`(a ,b)", &s, "b"), Region::Code);
        // Hash literals are sealed regardless of surroundings.
        assert_eq!(region_of("#(1 2 3)", &s, "#(1 2 3)"), Region::SealedData);
    }

    #[test]
    fn only_sealed_data_is_prunable() {
        assert!(Region::SealedData.is_prunable());
        assert!(!Region::PorousData.is_prunable());
        assert!(!Region::Code.is_prunable());
        // The binary bridge collapses both data kinds.
        assert_eq!(Region::SealedData.class(), Class::Data);
        assert_eq!(Region::PorousData.class(), Class::Data);
        assert_eq!(Region::Code.class(), Class::Code);
    }

    #[test]
    fn walk_class_matches_walk_regions_class_for_every_node() {
        // `walk`'s binary `Class` must equal `walk_regions`' `Region::class()`
        // node-for-node and in order. `walk` delegates to `walk_regions` today,
        // so this holds by construction — the test pins the contract so a future
        // reimplementation of either can't silently diverge (downstream tools
        // like cccc-scheme rely on `Region::class()` reproducing `Class` exactly).
        let cases = [
            (Options::scheme(), "(f x)"),
            (Options::scheme(), "'(a b)"),
            (Options::scheme(), "`(a ,b)"),
            (Options::scheme(), "``(,,c)"),
            (Options::scheme(), "'(a ,b)"),
            (Options::scheme(), "#(1 2 3)"),
            (Options::scheme(), "(let ((x 1)) `(v ,x '(w)))"),
            (Options::common_lisp(), "'(f #'a #.(g))"),
            (Options::common_lisp(), "#+sbcl (defun only () 1)"),
        ];
        for (opts, src) in &cases {
            let via_class = classes(src, opts);
            let via_region: Vec<(&str, Class)> = regions(src, opts)
                .into_iter()
                .map(|(t, r)| (t, r.class()))
                .collect();
            assert_eq!(via_class, via_region, "mismatch for {src:?}");
        }
    }

    #[test]
    fn blanket_skip_on_binary_data_drops_quasiquoted_code() {
        // The footgun: pruning on `Class::Data` skips the porous template whole,
        // so the unquoted `(f y)` — real code — is never seen.
        let s = Options::scheme();
        let src = "`(a ,(f y))";
        let parsed = parse(src, &s);
        let mut code_lists = Vec::new();
        walk(&parsed.data, |d, class| {
            if class == Class::Data {
                return Walk::Skip;
            }
            if matches!(d.kind, DatumKind::List { .. }) {
                code_lists.push(d.span.text(src));
            }
            Walk::Descend
        });
        // Bug reproduced: the unquoted call is lost.
        assert!(!code_lists.contains(&"(f y)"));
    }

    #[test]
    fn region_pruning_keeps_quasiquoted_code_but_prunes_sealed() {
        // The safe idiom: prune only prunable regions; descend porous templates.
        let s = Options::scheme();
        let src = "`(a ,(f y) '(b c))";
        let parsed = parse(src, &s);
        let mut code_lists = Vec::new();
        let mut visited = Vec::new();
        walk_regions(&parsed.data, |d, region| {
            visited.push(d.span.text(src));
            if region.is_prunable() {
                return Walk::Skip;
            }
            if region == Region::Code && matches!(d.kind, DatumKind::List { .. }) {
                code_lists.push(d.span.text(src));
            }
            Walk::Descend
        });
        // The unquoted call is reached...
        assert!(code_lists.contains(&"(f y)"));
        // ...while the sealed quoted list is pruned (visited once, not descended).
        assert!(visited.contains(&"(b c)"));
        assert!(!visited.contains(&"b"));
        assert!(!visited.contains(&"c"));
    }

    /// Every code node `code_nodes` yields, by source text, in order.
    fn code_texts<'a>(src: &'a str, opts: &Options) -> Vec<&'a str> {
        let parsed = parse(src, opts);
        code_nodes(&parsed.data).map(|d| d.span.text(src)).collect()
    }

    #[test]
    fn code_nodes_yields_code_preorder_and_prunes_sealed() {
        let s = Options::scheme();
        // The quote *form* `'(a b)` is code (it evaluates to data); its contents
        // are sealed and pruned. Pre-order: list, head, the quote form, tail.
        assert_eq!(
            code_texts("(f '(a b) x)", &s),
            vec!["(f '(a b) x)", "f", "'(a b)", "x"],
        );
    }

    #[test]
    fn code_nodes_descends_porous_to_reach_unquoted_code() {
        let s = Options::scheme();
        let got = code_texts("`(a ,(f y))", &s);
        // The unquoted call and its parts are reached...
        assert!(got.contains(&"(f y)"));
        assert!(got.contains(&"y"));
        // ...but template data at depth 1 (`a`) is not code, so not yielded.
        assert!(!got.contains(&"a"));
    }

    #[test]
    fn code_nodes_is_fused_after_exhaustion() {
        let s = Options::scheme();
        let parsed = parse("(f x)", &s);
        let mut it = code_nodes(&parsed.data);
        while it.next().is_some() {}
        // Drained stays drained — the FusedIterator contract.
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }

    #[test]
    fn code_nodes_is_lazy_and_short_circuits() {
        let s = Options::scheme();
        let parsed = parse("(a (b (c (d e))))", &s);
        // `find` stops as soon as it hits `b`; the rest is never walked.
        let first_b = code_nodes(&parsed.data).find(|d| d.span.text("(a (b (c (d e))))") == "b");
        assert!(first_b.is_some());
    }

    #[test]
    fn code_nodes_matches_walk_regions_code_classification() {
        // The iterator's fixed prune/descend policy must agree, node-for-node
        // and in order, with descending everything and keeping the code nodes.
        let s = Options::scheme();
        for src in [
            "(f x)",
            "`(a ,(f y) '(b c))",
            "(a '(b c) d)",
            "``(,,c)",
            "'(a ,b)",
            "(let ((x 1)) `(v ,x))",
        ] {
            let parsed = parse(src, &s);
            let via_iter: Vec<&str> = code_nodes(&parsed.data).map(|d| d.span.text(src)).collect();
            let mut via_visitor = Vec::new();
            walk_regions(&parsed.data, |d, r| {
                if r == Region::Code {
                    via_visitor.push(d.span.text(src));
                }
                Walk::Descend
            });
            assert_eq!(via_iter, via_visitor, "mismatch for {src:?}");
        }
    }

    #[test]
    fn skip_prunes_quoted_subtree() {
        let s = Options::scheme();
        let src = "(a '(big list) b)";
        let parsed = parse(src, &s);
        let mut visited = Vec::new();
        walk(&parsed.data, |d, class| {
            visited.push(d.span.text(src));
            if class == Class::Data {
                Walk::Skip
            } else {
                Walk::Descend
            }
        });
        // The quoted list is visited once and pruned; its items are not.
        assert!(visited.contains(&"'(big list)"));
        assert!(visited.contains(&"(big list)"));
        assert!(!visited.contains(&"big"));
        assert!(!visited.contains(&"list"));
        // Sibling code is still reached.
        assert!(visited.contains(&"b"));
    }
}
