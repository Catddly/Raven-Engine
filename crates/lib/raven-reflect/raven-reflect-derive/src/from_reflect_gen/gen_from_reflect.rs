use proc_macro::TokenStream;
use quote::quote;

use crate::quoted::{QuotedOption, QuotedClone};

use crate::reflect_meta::ReflectMeta;

pub(crate) fn gen_from_reflect_primitives(meta: &ReflectMeta) -> TokenStream {
    let type_name = meta.type_name();
    let reflect_crate_path = meta.reflect_crate_path();

    let (impl_generics, ty_generics, where_clause) = meta.generics().split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics #reflect_crate_path::FromReflect for #type_name #ty_generics #where_clause {
            fn from_reflect(reflected: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<Self> {
                #QuotedOption::Some(#QuotedClone::clone(<dyn #reflect_crate_path::Reflect>::downcast_ref::<#type_name #ty_generics>(reflected)?))
            }
        }
    })
}