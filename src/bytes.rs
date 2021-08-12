/// Read from `$rdr` and cast bytes into a struct of type `$ty`.
#[macro_export]
macro_rules! read_type {
    ($rdr: expr, $ty: ty) => {{
        // `size_of` and `transmute` cannot be easily used with generics.
        let mut buf = [0u8; std::mem::size_of::<$ty>()];

        $rdr.read(&mut buf)?;

        let hdr: $ty = unsafe { std::mem::transmute(buf) };

        let res: Result<$ty> = Ok(hdr);

        res
    }}
}