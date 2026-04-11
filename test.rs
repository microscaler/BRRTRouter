fn main() {
    let (tx, rx) = may::sync::mpsc::sync_channel::<i32>(10);
}
