use expressive_core;

#[test]
fn test_core_initialization() {
    // This is a basic integration test.
    // It verifies that the `expressive-core` crate can be imported
    // and used by an external crate (like the simulator or firmware).
    let is_ready = expressive_core::test_core();

    assert!(is_ready, "Core should successfully report as ready");
}
