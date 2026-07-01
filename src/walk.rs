//! Code-vs-data walker (ADR-0026).
//!
//! A best-effort *pruning visitor* over a [`Datum`] tree that classifies each
//! node as [`Class::Code`] or [`Class::Data`] and lets a consumer prune whole
//! subtrees (the search need: descend into code, skip quoted data). It is a
//! utility layer over the reader's tree — it interprets structure already
//! present and evaluates nothing (ADR-0001).
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
//! - `VarQuote`/`FunctionQuote` (`#'foo`), `Deref` (`@x`), `Splice`, `ReadEval`,
//!   and `HashFn` (`#(...)`) mark their contents as `Code` (unless under a hard
//!   `Quote`, which wins).
//! - `HashLiteral` (`#(1 2 3)`, `#u8(...)`, tagged `#inst …`), `LabelRef`
//!   (`#n#`), and `Discard` are `Data`.
//! - `Meta`, `Mutable`, and `Label` are **context-transparent** (inherit the
//!   parent's class); a `ReaderConditional`'s guarded form is transparent too.
//!
//! The visitor is primary because pruning cannot be expressed by a bare
//! iterator. A pre-order iterator adapter may be layered on later.

use crate::datum::{Datum, DatumKind, Prefix};

/// Whether a subtree is executable code or inert data (ADR-0026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Can be evaluated — descend into it for code analysis.
    Code,
    /// Inert data — a search prunes it.
    Data,
}

/// What the visitor callback asks the walker to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Walk {
    /// Recurse into this datum's children.
    Descend,
    /// Do not recurse into this datum's children.
    Skip,
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

    fn class(self) -> Class {
        if self.hard_quote || self.qq > 0 {
            Class::Data
        } else {
            Class::Code
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
        // Code references: back to code — unless a hard quote encloses them.
        Prefix::VarQuote
        | Prefix::FunctionQuote
        | Prefix::Deref
        | Prefix::Splice
        | Prefix::ReadEval
        | Prefix::HashFn => {
            if ctx.hard_quote {
                ctx
            } else {
                Ctx::TOP
            }
        }
        // Context-transparent wrappers.
        Prefix::Meta | Prefix::Mutable | Prefix::ReaderConditional(_) => ctx,
        // Discarded content is inert.
        Prefix::Discard => Ctx::DATA,
    }
}

/// The class of a datum given the region it occupies. `HashLiteral` and
/// `LabelRef` are inert data regardless of region.
fn node_class(datum: &Datum<'_>, ctx: Ctx) -> Class {
    match &datum.kind {
        DatumKind::HashLiteral { .. } | DatumKind::LabelRef { .. } => Class::Data,
        _ => ctx.class(),
    }
}

/// Walk each top-level datum in `data`, invoking `visit(datum, class)` in
/// pre-order. When the callback returns [`Walk::Skip`], that datum's children
/// are pruned; [`Walk::Descend`] recurses. Top-level data start as [`Class::Code`].
pub fn walk<'a, 't, F>(data: &'a [Datum<'t>], mut visit: F)
where
    F: FnMut(&'a Datum<'t>, Class) -> Walk,
{
    for datum in data {
        walk_datum(datum, Ctx::TOP, &mut visit);
    }
}

fn walk_datum<'a, 't, F>(datum: &'a Datum<'t>, ctx: Ctx, visit: &mut F)
where
    F: FnMut(&'a Datum<'t>, Class) -> Walk,
{
    if visit(datum, node_class(datum, ctx)) == Walk::Skip {
        return;
    }
    match &datum.kind {
        DatumKind::List { items, tail, .. } => {
            for item in items {
                walk_datum(item, ctx, visit);
            }
            if let Some(tail) = tail {
                walk_datum(tail, ctx, visit);
            }
        }
        DatumKind::Prefixed { prefix, inner, .. } => {
            walk_datum(inner, inner_ctx(*prefix, ctx), visit);
        }
        // A hash literal's content is data; a datum label is transparent.
        DatumKind::HashLiteral {
            inner: Some(inner), ..
        } => {
            walk_datum(inner, Ctx::DATA, visit);
        }
        DatumKind::Label { inner, .. } => {
            walk_datum(inner, ctx, visit);
        }
        _ => {}
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
