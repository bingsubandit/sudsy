fn f() -> impl Iterator<Item = u8> {
    gen {
        yield 1;
    }
}
