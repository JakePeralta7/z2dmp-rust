#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];  // ignore `f`

        let delim = "::";
        let vec: Vec<_> = name.split(delim).collect();

        vec[1..].join(delim)  // ignore module name
    }}
}