#[test]
#[cfg(feature = "backend")]
fn backend_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/macro_tests/compile/pass/backend/**/*.rs");
    t.compile_fail("tests/macro_tests/compile/fail/backend/**/*.rs");
}
