use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};

use quote::{quote, ToTokens};

use crate::{quoted::QuotedOption, reflect_meta::EnumMetaInfo, reflect_gen::{EnumVariantConstructors, get_variant_constructors}};

pub(crate) fn from_gen_enum(reflect_enum: &EnumMetaInfo) -> TokenStream {
    let qoption = QuotedOption.into_token_stream();

    let type_name = reflect_enum.meta().type_name();
    let reflect_crate_path = reflect_enum.meta().reflect_crate_path();

    let ref_value = Ident::new("__param0", Span::call_site());
    let EnumVariantConstructors {
        variant_names,
        variant_constructors,
    } = get_variant_constructors(reflect_enum, &ref_value, false);

    let (impl_generics, ty_generics, where_clause) =
        reflect_enum.meta().generics().split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics #reflect_crate_path::FromReflect for #type_name #ty_generics #where_clause  {
            fn from_reflect(#ref_value: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<Self> {
                if let #reflect_crate_path::ReflectRef::Enum(#ref_value) = #reflect_crate_path::Reflect::reflect_ref(#ref_value) {
                    match #reflect_crate_path::type_info::Enum::variant_name(#ref_value) {
                        #(#variant_names => #qoption::Some(#variant_constructors),)*
                        name => panic!("Variant with name `{}` does not exist on enum `{}`", name, ::core::any::type_name::<Self>()),
                    }
                } else {
                    #QuotedOption::None
                }
            }
        }
    })
}