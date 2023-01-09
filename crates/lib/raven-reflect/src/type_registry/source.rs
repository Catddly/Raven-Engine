use super::TypeRegistration;

/// Trait to be used in #[derive(Reflect)] to generate TypedMeta. 
pub trait FromType<T> {
    fn from_type() -> Self;
}

pub trait GetTypeRegistration {
    fn get_type_registration() -> TypeRegistration;
}