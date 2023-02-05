use bit_set::BitSet;
use proc_macro2::{TokenStream, Ident};
use syn::{Path, Generics};

use quote::quote;

pub(crate) fn gen_type_registration(
    type_name: &Ident,
    reflect_crate_path: &Path,
    trait_idents: &[Ident],
    generics: &Generics,
    serialization_denylist: Option<&BitSet<u32>>,
) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let serialization_data = serialization_denylist.map(|denylist| {
        let denylist = denylist.into_iter();
        quote! {
            let ignored_indices_iter = ::core::iter::IntoIterator::into_iter([#(#denylist),*]);
            registration.insert::<#reflect_crate_path::serde::SerializationData>(#reflect_crate_path::serde::SerializationData::new(ignored_indices_iter));
        }
    });

    quote! {
        #[allow(unused_mut)]
        impl #impl_generics #reflect_crate_path::type_registry::GetTypeRegistration for #type_name #ty_generics #where_clause {
            fn get_type_registration() -> #reflect_crate_path::type_registry::TypeRegistration {
                let mut registration = #reflect_crate_path::type_registry::TypeRegistration::type_of::<#type_name #ty_generics>();
                // ReflectFromPtr
                registration.insert::<#reflect_crate_path::type_registry::ReflectFromPtr>(
                    #reflect_crate_path::type_registry::FromType::<#type_name #ty_generics>::from_type()
                );
                // SerializationData
                #serialization_data
                // Reflected Traits (i.e. Default -> ReflectDefault, Serialize -> ReflectSerialize)
                #(registration.insert::<#trait_idents>(#reflect_crate_path::type_registry::FromType::<#type_name #ty_generics>::from_type());)*
                registration
            }
        }
    }
}