use proc_macro2::{TokenStream, Ident};
use quote::quote;
use syn::{Generics, Path};

pub(crate) fn gen_typed(
    type_name: &Ident,
    generics: &Generics,
    generator: proc_macro2::TokenStream,
    reflect_crate_path: &Path,
) -> TokenStream {
    let is_generics = !generics.params.is_empty();

    let static_cell = if is_generics {
        quote! {
            static TYPE_INFO_CELL: #reflect_crate_path::GenericTypeInfoOnceCell = #reflect_crate_path::GenericTypeInfoOnceCell::new();
            TYPE_INFO_CELL.get_or_insert::<Self, _>(|| { #generator })
        }
    } else {
        quote! {
            static TYPE_INFO_CELL: #reflect_crate_path::NonGenericTypeInfoOnceCell = #reflect_crate_path::NonGenericTypeInfoOnceCell::new();
            TYPE_INFO_CELL.get_or_set(|| { #generator })
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // generate a static OnceCell to store TypeInfo for `'static` lifetime.
    quote! {
        impl #impl_generics #reflect_crate_path::Typed for #type_name #ty_generics #where_clause {
            fn type_info() -> &'static #reflect_crate_path::TypeInfo {
                #static_cell
            }
        }
    }
}