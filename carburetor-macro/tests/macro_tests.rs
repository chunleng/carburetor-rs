use trybuild::TestCases;

#[test]
fn tests() {
    let t = TestCases::new();
    t.pass("tests/macro_tests/compile/pass/**/*.rs");
    t.compile_fail("tests/macro_tests/compile/fail/**/*.rs");
}
