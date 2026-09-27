#[test]
fn a_derive_without_grw_is_refused() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
