fn f(a: Option<u8>) -> u8 {
    if let Some(x) = a && x > 0 {
        x
    } else {
        0
    }
}
