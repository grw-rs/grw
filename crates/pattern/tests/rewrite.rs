use grw_pattern::{first_span, parse_node_arg, parse_with_clause, require_pinned, rewrite_clusters, split_with, ContextCtor, NodePat, NodeRef, Pinning};
use proc_macro2::{Span, TokenStream};
use quote::quote;

fn s(ts: TokenStream) -> String {
    ts.to_string()
}

#[test]
fn ints_pass_through() {
    let r = rewrite_clusters(quote!(get(Mono) { N(0) ^ N(1), N_() >> n(0) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) ^ N(1), N_() >> n(0) })));
    assert!(r.names.is_empty());
    assert!(r.context.is_empty());
}

#[test]
fn names_lower_to_indices_after_max_int() {
    let r = rewrite_clusters(quote!(get(Mono) { N(3) ^ N(i), n(i) >> N(j) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(3) ^ N(4), n(4) >> N(5) })));
    assert_eq!(r.names, vec![("i".to_string(), 4), ("j".to_string(), 5)]);
}

#[test]
fn names_start_at_zero_without_ints() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a) ^ N(b) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) ^ N(1) })));
}

#[test]
fn value_pattern_on_node() {
    let r = rewrite_clusters(quote!(get(Mono) { N(i: Some(10)) ^ N(j: Shape::Circle { r: 1..=5, .. }) })).unwrap();
    assert_eq!(
        s(r.body),
        s(quote!(get(Mono) {
            N(0).test(|__v| ::core::matches!(__v, Some(10)))
                ^ N(1).test(|__v| ::core::matches!(__v, Shape::Circle { r: 1..=5, .. }))
        }))
    );
}

#[test]
fn value_pattern_on_edge() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a) & E(Kind::Signs) >> N(b) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) & E().test(|__e| ::core::matches!(__e, Kind::Signs)) >> N(1) })));
}

#[test]
fn context_pin_and_pattern() {
    let r = rewrite_clusters(quote!(get(Mono) { X(f = fid : Function { critical: true, .. }) ^ N(a) })).unwrap();
    assert_eq!(
        s(r.body),
        s(quote!(get(Mono) { X(0).test(|__v| ::core::matches!(__v, Function { critical: true, .. })) ^ N(1) }))
    );
    assert!(!r.context.is_empty());
    assert_eq!(r.pins.len(), 1);
    assert_eq!(r.pins[0].lid, 0);
    assert_eq!(s(r.pins[0].expr.clone()), "fid");
}

#[test]
fn legacy_context_int_sets_flag_without_pin() {
    let r = rewrite_clusters(quote!(get(Mono) { X(0) ^ N(1) })).unwrap();
    assert!(!r.context.is_empty());
    assert!(r.pins.is_empty());
}

#[test]
fn duplicate_definition_in_one_cluster_is_error() {
    let e = rewrite_clusters(quote!(get(Mono) { N(i) ^ N(i) })).err().unwrap();
    assert!(e.to_string().contains("`i` defined twice"));
}

#[test]
fn same_name_across_clusters_is_one_node() {
    let r = rewrite_clusters(quote!(get(Mono) { N(i) ^ N(j) }, ban(Mono) { n(i) ^ N(k) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) ^ N(1) }, ban(Mono) { n(0) ^ N(2) })));
}

#[test]
fn undefined_reference_is_error() {
    let e = rewrite_clusters(quote!(get(Mono) { N(i) ^ n(zz) })).err().unwrap();
    assert!(e.to_string().contains("`zz` is never defined"));
}

#[test]
fn bad_pattern_is_error() {
    assert!(rewrite_clusters(quote!(get(Mono) { N(i: Some(=)) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { N(a) & E(==) >> N(b) })).is_err());
}

#[test]
fn node_arg_forms() {
    let a = parse_node_arg(quote!(i)).unwrap();
    assert!(matches!(a.id, Some(NodeRef::Name(ref n)) if n == "i"));
    let a = parse_node_arg(quote!(7 : Some(_))).unwrap();
    assert!(matches!(a.id, Some(NodeRef::Int(7))));
    assert!(a.pat.is_some());
    let a = parse_node_arg(quote!(f = fid : Foo { .. })).unwrap();
    assert!(a.pin.is_some() && a.pat.is_some());
    assert!(parse_node_arg(quote!()).unwrap().id.is_none());
}

#[test]
fn with_clause() {
    let w = parse_with_clause(quote!(X(f = fid), X(ff = other_fid)), Span::call_site()).unwrap();
    assert_eq!(w.pins.iter().map(|(n, e)| (n.as_str(), e.to_string())).collect::<Vec<_>>(), vec![("f", "fid".to_string()), ("ff", "other_fid".to_string())]);
    assert!(parse_with_clause(quote!(X(f = a), X(f = b)), Span::call_site()).err().unwrap().to_string().contains("`f` pinned twice"));
    assert!(parse_with_clause(quote!(N(f = a)), Span::call_site()).is_err());
}

#[test]
fn with_clause_wrong_delimiter_is_error() {
    assert!(parse_with_clause(quote!(X[f = a]), Span::call_site()).is_err());
}

#[test]
fn empty_id_is_error_not_panic() {
    assert!(rewrite_clusters(quote!(get(Mono) { N() ^ N(a) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { n() ^ N(a) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { X() ^ N(a) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { T() ^ N(a) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { x() ^ N(a) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { t() ^ N(a) })).is_err());
}

#[test]
fn cluster_head_expr_not_scanned_or_rewritten() {
    let r = rewrite_clusters(quote!(get(N(a)) { N(b) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(N(a)) { N(0) })));
    assert_eq!(r.names, vec![("b".to_string(), 0)]);
}

#[test]
fn closure_method_call_not_dsl() {
    let r = rewrite_clusters(quote!(get(Mono) { N(0).test(|v| helper.n(7)) ^ N(a) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0).test(|v| helper.n(7)) ^ N(1) })));
    assert_eq!(r.names, vec![("a".to_string(), 1)]);
}

#[test]
fn path_qualified_ctor_not_dsl() {
    let r = rewrite_clusters(quote!(get(Mono) { path::N(3) ^ N(a) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { path::N(3) ^ N(0) })));
    assert_eq!(r.names, vec![("a".to_string(), 0)]);
}

#[test]
fn colon_in_pin_survives_path() {
    let a = parse_node_arg(quote!(f = id::N(3) : Some(_))).unwrap();
    assert!(matches!(a.id, Some(NodeRef::Name(ref n)) if n == "f"));
    assert_eq!(a.pin.map(|p| p.to_string()), Some(quote!(id::N(3)).to_string()));
    let NodePat::Match(pat) = a.pat.unwrap() else { panic!("expected a plain match pattern") };
    assert_eq!(pat.to_string(), quote!(Some(_)).to_string());
}

#[test]
fn context_pin_expr_may_contain_path_colon() {
    let r = rewrite_clusters(quote!(get(Mono) { X(f = id::N(3) : Some(_)) ^ N(a) })).unwrap();
    assert_eq!(r.pins.len(), 1);
    assert_eq!(r.pins[0].expr.to_string(), quote!(id::N(3)).to_string());
    assert_eq!(s(r.body), s(quote!(get(Mono) { X(0).test(|__v| ::core::matches!(__v, Some(_))) ^ N(1) })));
}

#[test]
fn node_pattern_may_contain_path() {
    let r = rewrite_clusters(quote!(get(Mono) { N(i: Kind::A) ^ N(j) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0).test(|__v| ::core::matches!(__v, Kind::A)) ^ N(1) })));
}

#[test]
fn edge_empty_passes_through() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a) & E() >> N(b) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) & E() >> N(1) })));
}

#[test]
fn int_id_node_pattern_still_lowers() {
    let r = rewrite_clusters(quote!(get(Mono) { N(0: Some(_)) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0).test(|__v| ::core::matches!(__v, Some(_))) })));
}

#[test]
fn int_id_context_pattern_still_lowers() {
    let r = rewrite_clusters(quote!(get(Mono) { X(0: Some(_)) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { X(0).test(|__v| ::core::matches!(__v, Some(_))) })));
}

#[test]
fn int_id_context_pin_still_lowers() {
    let r = rewrite_clusters(quote!(get(Mono) { X(0 = e) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { X(0) })));
    assert_eq!(r.pins.len(), 1);
    assert_eq!(r.pins[0].lid, 0);
    assert_eq!(r.pins[0].expr.to_string(), quote!(e).to_string());
}

#[test]
fn plain_int_node_is_unchanged() {
    let r = rewrite_clusters(quote!(get(Mono) { N(0) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) })));
}

#[test]
fn edge_with_pattern_vs_empty() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a) & E(1) >> N(b) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) & E().test(|__e| ::core::matches!(__e, 1)) >> N(1) })));
    let r = rewrite_clusters(quote!(get(Mono) { N(a) & E() >> N(b) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0) & E() >> N(1) })));
}

#[test]
fn context_t_alias_and_lowercase_refs() {
    let r = rewrite_clusters(quote!(get(Mono) { T(f = fid) ^ N(a) }, ban(Mono) { x(f) ^ N(b) })).unwrap();
    assert!(!r.context.is_empty());
    assert_eq!(r.pins.len(), 1);
    assert_eq!(r.pins[0].lid, 0);
    assert_eq!(s(r.body), s(quote!(get(Mono) { T(0) ^ N(1) }, ban(Mono) { x(0) ^ N(2) })));
}

#[test]
fn context_from_lowercase_int_ref_alone() {
    let r = rewrite_clusters(quote!(get(Mono) { N(0) ^ t(1) })).unwrap();
    assert!(!r.context.is_empty());
}

#[test]
fn bad_pin_expr_is_error() {
    assert!(rewrite_clusters(quote!(get(Mono) { X(f = +) ^ N(a) })).is_err());
}

#[test]
fn ref_to_context_only_name_resolves() {
    let r = rewrite_clusters(quote!(get(Mono) { X(f) ^ n(f) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { X(0) ^ n(0) })));
}

#[test]
fn ref_rejects_pin_and_pattern() {
    assert!(rewrite_clusters(quote!(get(Mono) { N(a) ^ n(a = 1) })).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { N(a) ^ n(a: Some(1)) })).is_err());
}

#[test]
fn suffixed_literal_is_error() {
    assert!(parse_node_arg(quote!(0u32)).is_err());
    assert!(rewrite_clusters(quote!(get(Mono) { N(0u32) ^ N(a) })).is_err());
}

#[test]
fn context_marks_each_context_ctor() {
    let bodies = [
        quote!(get(Mono) { X(a) ^ N(b) }),
        quote!(get(Mono) { T(a) ^ N(b) }),
        quote!(get(Mono) { N(a) ^ x(a) }),
        quote!(get(Mono) { N(a) ^ t(a) }),
    ];
    for body in bodies {
        assert!(!rewrite_clusters(body).unwrap().context.is_empty());
    }
    assert!(rewrite_clusters(quote!(get(Mono) { N(a) ^ n(a) })).unwrap().context.is_empty());
}

#[test]
fn empty_with_clause_is_error() {
    let e = parse_with_clause(quote!(), Span::call_site()).err().unwrap();
    assert!(e.to_string().contains("after `with`"));
}

#[test]
fn first_span_of_empty_stream_is_none() {
    assert!(first_span(&quote!()).is_none());
    assert!(first_span(&quote!(a b)).is_some());
}

#[test]
fn split_with_only_splits_the_keyword() {
    assert!(split_with(quote!(foo::with(1))).1.is_none());
    assert!(split_with(quote!(p.with(x))).1.is_none());
    assert!(split_with(quote!(p)).1.is_none());
    let (pat, w) = split_with(quote!(p with X(a = b)));
    assert_eq!(s(pat), s(quote!(p)));
    assert_eq!(s(w.unwrap().1), s(quote!(X(a = b))));
    let (pat, w) = split_with(quote!(cif() with));
    assert_eq!(s(pat), s(quote!(cif())));
    assert!(w.unwrap().1.is_empty());
}

#[test]
fn cluster_head_needs_paren_brace_and_no_access_prefix() {
    let toks: Vec<proc_macro2::TokenTree> = quote!(get(Mono) { N(a) }).into_iter().collect();
    assert!(grw_pattern::is_cluster_head(None, &toks));
    let dot = proc_macro2::TokenTree::Punct(proc_macro2::Punct::new('.', proc_macro2::Spacing::Alone));
    assert!(!grw_pattern::is_cluster_head(Some(&dot), &toks));
    let no_brace: Vec<proc_macro2::TokenTree> = quote!(get(Mono)).into_iter().collect();
    assert!(!grw_pattern::is_cluster_head(None, &no_brace));
    let not_head: Vec<proc_macro2::TokenTree> = quote!(fetch(Mono) { N(a) }).into_iter().collect();
    assert!(!grw_pattern::is_cluster_head(None, &not_head));
}

#[test]
fn key_on_node() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a: key(BY_MOD, 3u32)) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0).key(BY_MOD, 3u32) })));
}

#[test]
fn key_in_on_node() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a: key_in(BY_MOD, [1u32, 2u32])) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { N(0).key_in(BY_MOD, [1u32, 2u32]) })));
}

#[test]
fn pin_and_key_on_context() {
    let r = rewrite_clusters(quote!(get(Mono) { X(c = one : key(BY_MOD, 3u32)) ^ N(a) })).unwrap();
    assert_eq!(s(r.body), s(quote!(get(Mono) { X(0).key(BY_MOD, 3u32) ^ N(1) })));
    assert_eq!(r.pins.len(), 1);
    assert_eq!(r.pins[0].lid, 0);
    assert_eq!(s(r.pins[0].expr.clone()), "one");
}

#[test]
fn key_inside_larger_pattern_is_not_recognized() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a: key(BY_MOD, 3u32) | other) })).unwrap();
    assert_eq!(
        s(r.body),
        s(quote!(get(Mono) { N(0).test(|__v| ::core::matches!(__v, key(BY_MOD, 3u32) | other)) }))
    );
}

#[test]
fn key_on_edge_is_rejected() {
    let e = rewrite_clusters(quote!(get(Mono) { N(a) & E(key(BY_MOD, 3u32)) >> N(b) })).unwrap_err();
    assert_eq!(e.to_string(), "key predicates apply to node definitions");
}

#[test]
fn key_on_node_ref_is_rejected() {
    let e = rewrite_clusters(quote!(get(Mono) { N(a) ^ n(a: key(BY_MOD, 3u32)) })).unwrap_err();
    assert_eq!(e.to_string(), "key predicates apply to node definitions");
}

#[test]
fn key_on_context_ref_is_rejected() {
    let e = rewrite_clusters(quote!(get(Mono) { X(a) ^ x(a: key(BY_MOD, 3u32)) })).unwrap_err();
    assert_eq!(e.to_string(), "key predicates apply to node definitions");
    let e = rewrite_clusters(quote!(get(Mono) { T(a) ^ t(a: key(BY_MOD, 3u32)) })).unwrap_err();
    assert_eq!(e.to_string(), "key predicates apply to node definitions");
}

#[test]
fn malformed_key_missing_comma_is_error() {
    assert!(rewrite_clusters(quote!(get(Mono) { N(a: key(BY_MOD)) })).is_err());
}

#[test]
fn malformed_key_in_missing_comma_is_error() {
    assert!(rewrite_clusters(quote!(get(Mono) { N(a: key_in(BY_MOD)) })).is_err());
}

#[test]
fn a_pinned_context_node_is_pinned() {
    let r = rewrite_clusters(quote!(get(Mono) { X(a = zero) ^ N(b) })).unwrap();
    assert_eq!(r.context.len(), 1);
    assert_eq!(r.context[0].lid, 0);
    assert_eq!(r.context[0].pin, Pinning::Pinned);
    assert!(require_pinned(&r).is_ok());
}

#[test]
fn an_unpinned_context_node_is_refused_by_name() {
    let r = rewrite_clusters(quote!(get(Mono) { X(c = zero) ^ X(a) ^ N(b) })).unwrap();
    assert_eq!(r.context.iter().map(|c| c.pin).collect::<Vec<_>>(), vec![Pinning::Pinned, Pinning::Unpinned]);
    assert_eq!(r.context[1].ctor, ContextCtor::X);
    let err = require_pinned(&r).unwrap_err().to_string();
    assert_eq!(err, "context node `a` is not pinned to a graph node: write `X(a = <expr>)`");
}

#[test]
fn an_unpinned_integer_context_node_is_refused_by_its_id() {
    let r = rewrite_clusters(quote!(get(Mono) { X(0) ^ N(1) })).unwrap();
    let err = require_pinned(&r).unwrap_err().to_string();
    assert_eq!(err, "context node `0` is not pinned to a graph node: write `X(0 = <expr>)`");
}

#[test]
fn a_translated_node_shares_the_context_rule_and_is_named_as_written() {
    let r = rewrite_clusters(quote!(get(Mono) { T(0) ^ N(1) })).unwrap();
    assert_eq!(r.context[0].pin, Pinning::Unpinned);
    assert_eq!(r.context[0].ctor, ContextCtor::T);
    let err = require_pinned(&r).unwrap_err().to_string();
    assert_eq!(err, "context node `0` is not pinned to a graph node: write `T(0 = <expr>)`");
}

#[test]
fn a_context_node_pinned_in_one_cluster_is_pinned_in_all() {
    let r = rewrite_clusters(quote!(get(Mono) { X(a = zero) ^ N(b) }, ban(Mono) { X(a) ^ N(c) })).unwrap();
    assert_eq!(r.context.len(), 1);
    assert_eq!(r.context[0].pin, Pinning::Pinned);
    assert!(require_pinned(&r).is_ok());
}

#[test]
fn a_context_reference_names_the_same_node() {
    let r = rewrite_clusters(quote!(get(Mono) { X(a = zero) ^ N(b), x(a) ^ N(c) })).unwrap();
    assert_eq!(r.context.len(), 1);
    assert_eq!(r.context[0].pin, Pinning::Pinned);
}

#[test]
fn free_nodes_carry_no_context() {
    let r = rewrite_clusters(quote!(get(Mono) { N(a) ^ N(b) })).unwrap();
    assert!(r.context.is_empty());
    assert!(require_pinned(&r).is_ok());
}
