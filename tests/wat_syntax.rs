//! Broad WAT/WAST syntax coverage, derived from the text-format grammar in
//! `WebAssembly/spec` (`interpreter/README.md#s-expression-syntax` and
//! <https://webassembly.github.io/spec/core/text/index.html>).
//!
//! Every snippet here is accepted by the reference interpreter. The reader is
//! allowed to be a *superset* (it does not validate), but it must not lose sync
//! or reject valid syntax.

use lispexp::{lex, parse, Options};

fn wat() -> Options {
    Options::wat()
}

fn assert_clean(name: &str, source: &str) {
    let parsed = parse(source, &wat());
    assert!(
        parsed.errors.is_empty(),
        "{name}: parse errors: {:?}\nsource: {source}",
        parsed.errors
    );

    // The lexer's tiling contract: tokens cover the source exactly, in order.
    let mut offset = 0usize;
    for token in lex(source, &wat()) {
        assert_eq!(token.span.start as usize, offset, "{name}: tiling gap");
        assert!(token.span.end >= token.span.start, "{name}: inverted span");
        offset = token.span.end as usize;
    }
    assert_eq!(
        offset,
        source.len(),
        "{name}: lexer did not tile the source"
    );
}

#[test]
fn cr_only_newlines() {
    // WAT's `newline` is LF, CR, or CRLF. A lone CR must terminate a line
    // comment, and separate tokens like any other whitespace.
    assert_clean("cr between forms", "(module)\r(func)");
    assert_clean("cr line comment", "(module\r;; comment\r(func))");
    assert_clean("crlf line comment", "(module\r\n;; comment\r\n(func))");
    assert_clean("cr at eof", ";; comment\r");
}

#[test]
fn cr_recovery_after_unterminated_string() {
    // The same WAT newline set drives the unterminated-string backtrack: the
    // line-start `(c 1)` after a lone CR must be recovered, not lost.
    let parsed = parse("(a (b \"unterminated\r(c 1)", &wat());
    let mut found = false;
    fn walk(d: &lispexp::DatumKind<'_>, found: &mut bool) {
        if let lispexp::DatumKind::List { items, .. } = d {
            if items.first().and_then(|i| i.as_symbol()) == Some("c") {
                *found = true;
            }
            for i in items {
                walk(&i.kind, found);
            }
        }
    }
    for d in &parsed.data {
        walk(&d.kind, &mut found);
    }
    assert!(found, "`(c 1)` after a CR break must be recovered");
}

#[test]
fn module_fields_and_abbreviations() {
    let snippets = [
        // type / typedef
        "(module (type $t (func (param i32 i64) (result f32 f64))))",
        "(module (type (func)))",
        // import
        r#"(module (import "m" "f" (func $f (param i32) (result i32))))"#,
        r#"(module (import "m" "g" (global $g (mut i32))))"#,
        r#"(module (import "m" "t" (table $t 1 2 funcref)))"#,
        r#"(module (import "m" "mem" (memory $mem 1 2)))"#,
        // func abbreviations: inline export / import
        r#"(module (func $f (export "f") (param $x i32) (result i32) local.get $x))"#,
        r#"(module (func $f (import "m" "f") (param i32)))"#,
        // params / results / locals
        "(module (func (param i32 i64 f32 f64 v128) (param $x i32) (result i32) (local i32) (local $l f64) i32.const 0))",
        // global / table / memory
        "(module (global $g (mut i32) (i32.const 1)))",
        "(module (global i32 (i32.const 1)))",
        "(module (table $t 0 1 funcref))",
        "(module (table $t (ref null func)))",
        "(module (table $t 1 (ref null $t)))",
        "(module (memory $m 1))",
        "(module (memory $m 1 2 shared))",
        // elem / data / start
        "(module (elem (i32.const 0) $f $g))",
        "(module (elem (table $t) (offset (i32.const 0)) func $f))",
        "(module (elem declare func $f))",
        "(module (elem declare funcref (ref.func $f)))",
        "(module (elem funcref (ref.func $f) (ref.null func)))",
        r#"(module (data (i32.const 0) "a" "b"))"#,
        r#"(module (data (memory $m) (offset (i32.const 0)) "x"))"#,
        "(module (start $f))",
        // inline export / data abbreviations on table/memory/global
        r#"(module (table $t (export "t") funcref (elem $f $g)))"#,
        r#"(module (memory $m (export "m") (data "x" "y")))"#,
        r#"(module (global $g (export "g") i32 (i32.const 0)))"#,
        // export
        r#"(module (export "f" (func $f)))"#,
        r#"(module (export "g" (global $g)))"#,
        r#"(module (export "t" (table $t)))"#,
        r#"(module (export "m" (memory $m)))"#,
        // tag (exceptions)
        "(module (tag $e (param i32)))",
        "(module (tag $e (type $t)))",
        // module fields may appear in any order (ordering is a validation rule,
        // not a parse rule)
        r#"(module (func $f) (type $t (func)) (memory 1) (import "m" "g" (func $g)))"#,
    ];
    for (i, source) in snippets.iter().enumerate() {
        assert_clean(&format!("module field #{i}"), source);
    }
}

#[test]
fn types_and_reference_types() {
    let snippets = [
        "(module (type (func (param (ref null func)) (result (ref func)))))",
        "(module (type (func (param (ref null $t)) (result (ref $t)))))",
        "(module (type (func (param (ref null nofunc)) (result (ref none)))))",
        "(module (type (func (param anyref eqref i31ref dataref arrayref funcref externref))))",
        "(module (type (func (param nullref nullfuncref nullexternref))))",
        "(module (type (func (param (rtt $t)))))",
        "(module (type (func (param (mut i32)))))",
        "(module (type (rec (type $a (func)) (type $b (func)))))",
        "(module (type $t (sub $u (func))))",
        "(module (type $t (sub final $u (func))))",
        "(module (type $t (struct (field $x (mut i32)) (field f64))))",
        "(module (type $t (array (mut i8))))",
        "(module (type $t (func (param i32) (result i32))))",
    ];
    for (i, source) in snippets.iter().enumerate() {
        assert_clean(&format!("type #{i}"), source);
    }
}

#[test]
fn instructions_folded_and_linear() {
    let snippets = [
        // control, folded
        "(module (func (block $l (result i32) (i32.const 1))))",
        "(module (func (loop $l (br $l))))",
        "(module (func (if $l (result i32) (then (i32.const 1)) (else (i32.const 0)))))",
        "(module (func (i32.const 1) (if (then (nop)) (else (nop)))))",
        "(module (func (try_table (catch $e $l) (catch_ref $e $l) (catch_all $l) (catch_all_ref $l) (nop))))",
        // control, linear with end
        "(module (func block $l (result i32) i32.const 1 end $l))",
        "(module (func loop $l br $l end $l))",
        "(module (func i32.const 1 if $l (then) end $l))",
        // parametric / variable / table
        "(module (func (param $x i32) unreachable nop drop select (local.get $x) (local.set $x) (local.tee $x) (global.get $g) (global.set $g)))",
        "(module (func (select (result i32)) (table.get $t) (table.set $t) (table.size $t) (table.grow $t) (table.fill $t) (table.copy $t $t) (table.init $t $e) (elem.drop $e)))",
        // memory
        "(module (func (i32.load offset=4 align=4) (i32.load8_s) (i32.load16_u offset=0x10) (i64.load32_s) (f32.load) (f64.store) (v128.load) (i32.store8 offset=1 align=1) (memory.size) (memory.grow) (memory.fill) (memory.copy) (memory.init $d) (data.drop $d)))",
        // reference / aggregate
        "(module (func (ref.null func) (ref.func $f) (ref.is_null) (ref.as_non_null) (ref.test $t) (ref.cast $t) (ref.eq) (i31.new) (i31.get_s) (i31.get_u)))",
        "(module (func (struct.new $t) (struct.new_default $t) (struct.get $t $f) (struct.get_s $t $f) (struct.set $t $f) (array.new $t) (array.new_default $t) (array.new_fixed $t 2) (array.new_elem $t $e) (array.new_data $t $d) (array.get $t) (array.get_s $t) (array.set $t) (array.len $t) (extern.internalize) (extern.externalize)))",
        // numeric
        "(module (func i32.const -2147483648 i32.const 0xffffffff i64.const 0xffffffffffffffff f32.const 0x1p-149 f64.const 0x1.fffffffffffffp+1023 i32.add i32.sub i32.mul i32.ctz i32.clz i32.popcnt i32.eqz i32.eq i32.lt_s i32.trunc_f32_s i64.extend_i32_u f32.convert_i64_u f64.promote_f32))",
        "(module (func f32.const -inf f64.const nan f32.const nan:0x400000 f64.const -nan:0x1))",
        // vector
        "(module (func (v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15) (i8x16.shuffle 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15) (i8x16.abs) (i16x8.add) (i32x4.bitselect) (f32x4.eq) (i8x16.extract_lane_s 0) (i8x16.replace_lane 0)))",
        // block types
        "(module (func (block (result i32) (i32.const 1)) (block (param i32) (result i32) (i32.const 1)) (block (type $t))))",
        // call forms
        "(module (func (call $f) (call_indirect (type $t)) (call_indirect $t (type $t)) (return_call $f) (return_call_indirect (type $t)) (return_call_ref $t) (call_ref $t)))",
        "(module (func (br_table 0 1 2) (br_on_null 0) (br_on_non_null 0) (br_on_cast 0 (ref null func) (ref func)) (br_on_cast_fail 0 (ref null func) (ref func)) (throw $e) (throw_ref)))",
        "(module (func (v128.load8_splat) (v128.load16_lane) (v128.load32_zero) (v128.store8_lane) (v128.store16_lane)))",
        // labels as named vars in folded + linear
        "(module (func (block $b (br $b)) block $b2 br $b2 end $b2))",
    ];
    for (i, source) in snippets.iter().enumerate() {
        assert_clean(&format!("instruction #{i}"), source);
    }
}

#[test]
fn script_syntax_from_the_reference_interpreter() {
    let snippets = [
        // module / binary / quote / definition / instance
        "(module)",
        r#"(module $m)"#,
        r#"(module binary "\00asm\01\00\00\00")"#,
        r#"(module quote "(module)")"#,
        "(module definition $m binary \"\\00asm\\01\\00\\00\\00\")",
        "(module instance $i $m)",
        "(module instance $m)",
        // register / action / const
        r#"(register "m")"#,
        r#"(register "m" $m)"#,
        r#"(invoke "f")"#,
        r#"(invoke $m "f" (i32.const 1) (f64.const 1.5) (ref.null func) (ref.host 0) (ref.extern 1))"#,
        r#"(invoke "f" (v128.const i32x4 1 2 3 4))"#,
        r#"(get "g")"#,
        r#"(get $m "g")"#,
        // assertions and patterns
        r#"(assert_return (invoke "f") (i32.const 1) (i64.const 2))"#,
        r#"(assert_return (invoke "f") (f32.const nan:canonical) (f64.const nan:arithmetic))"#,
        r#"(assert_return (invoke "f") (v128.const i32x4 0 0 0 0) (ref) (ref.null) (ref.func) (ref.extern) (ref.any) (either (i32.const 1) (i32.const 2)))"#,
        r#"(assert_exception (invoke "f"))"#,
        r#"(assert_trap (invoke "f") "boom")"#,
        r#"(assert_exhaustion (invoke "f") "stack")"#,
        r#"(assert_malformed (module quote "(module") "unexpected")"#,
        r#"(assert_invalid (module (func (result i32))) "type mismatch")"#,
        r#"(assert_unlinkable (module instance) "link")"#,
        r#"(assert_trap (module instance) "instantiate")"#,
        // meta
        r#"(script $s (module) (invoke "f"))"#,
        r#"(input "a.wast")"#,
        r#"(output "out.wast")"#,
        r#"(output)"#,
        // A whole multi-command script, not one command at a time.
        r#"(module $m (func $f (export "f") (result i32) i32.const 1))
(register "m" $m)
(assert_return (invoke $m "f") (i32.const 1))
(assert_trap (invoke $m "missing") "unknown")"#,
    ];
    for (i, source) in snippets.iter().enumerate() {
        assert_clean(&format!("script #{i}"), source);
    }
}

#[test]
fn lexical_forms() {
    let snippets = [
        // comments
        ";;\n(module)",
        ";; no newline at eof",
        "(;;) (module)",
        "(; a ;) (module)",
        "(; a (; b ;) c ;) (module)",
        "(; (; (; ;) ;) ;) (module)",
        "(; ;; line-looking ;) (module)",
        // identifiers
        "(module $abc $ABC $0 $a.b $a+b $a-b $a*b $a/b $a\\b $a^b $a~b $a=b $a<b $a>b $a!b $a?b $a@b $a#b $a$b $a%b $a&b $a|b $a:b $a'b $a`b)",
        r#"(module $"" $"" "" )"#,
        // reserved tokens are read, not rejected (ADR-0030)
        "(module 0$x 'a'b' $)",
        // numbers (from the text values grammar)
        "(module (func i32.const 0 i32.const 1_000 i32.const 0x2A i32.const 0x1_000 i32.const +1 i32.const -1 i32.const +0x1 i32.const -0x1))",
        "(module (func f32.const 1. f32.const 1.5 f32.const 1.5e10 f32.const 1.5e+10 f32.const 1.5e-10 f32.const 1.e3 f32.const 0x1p3 f32.const 0x1.8p3 f32.const 0x1.p3 f32.const 0x1.ap3 f32.const 0x1.8p+3 f32.const inf f32.const -inf f32.const nan f32.const -nan f32.const nan:0x1 f32.const nan:0x1_2))",
        "(module (func f32.const 1_0.0_1E1_0 f64.const 0x1_0.0_1P1_0 f32.const -0x1.8P-3 f64.const +nan:0xABC_DEF))",
        // memory args
        "(module (func (i32.load offset=0 align=1) (i32.load offset=0x10 align=2) (i32.load offset=1_000 align=8)))",
        // strings: every escape the grammar allows
        r#"(data (i32.const 0) "a\n\t\r\\\'\"\22\u{1F600}\u{41}")"#,
        // annotations
        r#"(module (@custom "name" "data") (@custom2 (nested)) (func))"#,
        r#"(module (@"quoted id" "data"))"#,
        r#"(module (@custom $x))"#,
        r#"(module (@custom (; comment ;) (@nested) "x"))"#,
        // quoted identifiers use the same string escapes
        r#"(module (func $"\22\u{41}\" quoted"))"#,
    ];
    for (i, source) in snippets.iter().enumerate() {
        assert_clean(&format!("lexical #{i}"), source);
    }
}
