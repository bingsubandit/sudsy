fn f<'a>(x: &'a u8) -> impl Sized + use<'a> {
    x
}
