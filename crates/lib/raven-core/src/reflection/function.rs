/// get the name of a function at runtime.
pub fn get_function_name<F>(_: &F) -> &'static str
{
    std::any::type_name::<F>()
}