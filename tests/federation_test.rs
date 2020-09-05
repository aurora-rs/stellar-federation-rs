use stellar_federation::resolve_stellar_address;

#[tokio::test]
async fn test_federation() {
    let no_memo = resolve_stellar_address("francesco*ceccon.me")
        .await
        .unwrap();

    assert_eq!(
        no_memo.account_id.to_string(),
        "GBUFHFEIMKTBQQFDSCAZFOC6MAUE3EHBVE4S4RYKMX62PMWDIDSD44CP"
    );

    let with_memo_text = resolve_stellar_address("with-text-memo*ceccon.me")
        .await
        .unwrap();

    assert!(with_memo_text.memo.unwrap().is_text());

    let with_memo_id = resolve_stellar_address("with-id-memo*ceccon.me")
        .await
        .unwrap();

    assert!(with_memo_id.memo.unwrap().is_id());

    /*
    let with_memo_hash = resolve_stellar_address("with-hash-memo*ceccon.me")
        .await
        .unwrap();

    assert!(with_memo_hash.memo.unwrap().is_hash());
    */
}
