fn f(o: Option<u8>) -> u8 {
    let Some(x) = o else { return 0 };
    x
}
