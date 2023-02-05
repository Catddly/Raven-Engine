use proc_macro::TokenStream;
use quote::quote;

use crate::{reflect_meta::ReflectMeta, quoted::{QuotedOption, QuotedClone, QuotedAny}};

/// Implements `FromReflect` for the given value type
pub(crate) fn from_gen_primitive(meta: &ReflectMeta) -> TokenStream {
    let type_name = meta.type_name();
    let reflect_crate_path = meta.reflect_crate_path();
    let (impl_generics, ty_generics, where_clause) = meta.generics().split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics #reflect_crate_path::FromReflect for #type_name #ty_generics #where_clause  {
            fn from_reflect(reflect: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<Self> {
                #QuotedOption::Some(#QuotedClone::clone(<dyn #QuotedAny>::downcast_ref::<#type_name #ty_generics>(<dyn #reflect_crate_path::Reflect>::as_any(reflect))?))
            }
        }
    })
}