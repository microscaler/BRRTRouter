use brrtrouter::sse;

#[test]
fn test_channel_single_message() {
    let (tx, rx) = sse::channel();
    tx.send("hello");
    drop(tx);
    let result = rx.collect();
    assert_eq!(result, "data: hello\n\n");
}

#[test]
fn test_channel_multiple_messages() {
    let (tx, rx) = sse::channel();
    tx.send("first");
    tx.send("second");
    drop(tx);
    let result = rx.collect();
    assert_eq!(result, "data: first\n\ndata: second\n\n");
}
