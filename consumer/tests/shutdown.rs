use consumer::shutdown::Shutdown;

#[tokio::test]
async fn cancel_propagates_to_all_clones() {
    let s = Shutdown::new();
    let t1 = s.token();
    let t2 = s.token();
    assert!(!t1.is_cancelled());
    t1.cancel();
    assert!(t2.is_cancelled());
}