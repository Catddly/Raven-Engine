use proc_macro::TokenStream;
use quote::quote;

use crate::reflect_meta::{ReflectMeta};
use crate::{quoted::{QuotedBox, QuotedAny}};
use super::gen_typed;

pub(crate) fn gen_primitives(prim_meta: &ReflectMeta) -> TokenStream {
    let reflect_crate_path = prim_meta.reflect_crate_path();

    let type_name = prim_meta.type_name();
    let generics = prim_meta.generics();

    let impl_typed = gen_typed(
        type_name,
        generics,
        quote! {
            let type_info = #reflect_crate_path::type_info::PrimitiveTypeInfo::new::<Self>();
            #reflect_crate_path::type_info::TypeInfo::Primitive(type_info)
        },
        reflect_crate_path,
    );

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    TokenStream::from(
        quote! {
            // implement Typed
            #impl_typed

            // implement Reflect
            impl #impl_generics #reflect_crate_path::Reflect for #type_name #type_generics #where_clause {
                #[inline]
                fn type_name(&self) -> &'static str {
                    ::core::any::type_name::<Self>()
                }
            
                #[inline]
                fn into_any(self: #QuotedBox<Self>) -> #QuotedBox<dyn #QuotedAny> {
                    self
                }

                #[inline]
                fn as_any(&self) -> &dyn #QuotedAny {
                    self
                }

                #[inline]
                fn as_any_mut(&mut self) -> &mut dyn #QuotedAny {
                    self
                }
            
                #[inline]
                fn into_reflect(self: #QuotedBox<Self>) -> #QuotedBox<dyn #reflect_crate_path::Reflect> {
                    self
                }

                #[inline]
                fn as_reflect(&self) -> &dyn #reflect_crate_path::Reflect {
                    self
                }

                #[inline]
                fn as_reflect_mut(&mut self) -> &mut dyn #reflect_crate_path::Reflect {
                    self
                }
            }
        }
    )
}