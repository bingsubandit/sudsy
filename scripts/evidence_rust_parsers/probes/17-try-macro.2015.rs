fn f() -> Result<u8, ()> {
    let x = try!(Ok(1));
    Ok(x)
}
