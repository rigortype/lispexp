//! Options/CharRoles spot-checks.

use lispexp::{CharRoles, Dialect, Options};

#[test]
fn janet_roles_splice_and_mutable() {
    let opts = Options::for_dialect(Dialect::Janet);
    assert_eq!(opts.roles.splice, Some(';'));
    assert_eq!(opts.roles.mutable, Some('@'));
    assert_eq!(opts.roles.quasiquote, Some('~'));
    assert_eq!(opts.roles.short_fn, Some('|'));
}

#[test]
fn clojure_roles_deref_and_meta() {
    let opts = Options::for_dialect(Dialect::Clojure);
    assert_eq!(opts.roles.deref, Some('@'));
    assert_eq!(opts.roles.meta, Some('^'));
    assert_eq!(opts.roles.unquote, Some('~'));
}

#[test]
fn scheme_base_has_no_clojure_extras() {
    let roles = CharRoles::scheme();
    assert_eq!(roles.deref, None);
    assert_eq!(roles.meta, None);
    assert_eq!(roles.splice, None);
    assert_eq!(roles.mutable, None);
    assert_eq!(roles.short_fn, None);
    assert_eq!(roles.quote, Some('\''));
    assert_eq!(roles.quasiquote, Some('`'));
    assert_eq!(roles.unquote, Some(','));
    assert_eq!(roles.splicing_suffix, '@');
}

#[test]
fn clojure_base_extends_scheme_quote_family() {
    let roles = CharRoles::clojure();
    assert_eq!(roles.quote, Some('\''));
    assert_eq!(roles.quasiquote, Some('`'));
    assert_eq!(roles.unquote, Some('~'));
    assert_eq!(roles.deref, Some('@'));
    assert_eq!(roles.meta, Some('^'));
}

#[test]
fn edn_has_no_quote_family_glyphs() {
    let opts = Options::for_dialect(Dialect::Edn);
    assert_eq!(opts.roles.quote, None);
    assert_eq!(opts.roles.quasiquote, None);
    assert_eq!(opts.roles.unquote, None);
    assert_eq!(opts.roles.deref, None);
    assert_eq!(opts.roles.meta, None);
}
