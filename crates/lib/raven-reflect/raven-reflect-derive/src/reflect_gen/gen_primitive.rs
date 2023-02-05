use proc_macro::TokenStream;
use quote::quote;

use crate::reflect_meta::{ReflectMeta};
use crate::{quoted::{QuotedBox, QuotedClone, QuotedOption}};
use super::gen_typed;

pub(crate) fn gen_primitive(prim_meta: &ReflectMeta) -> TokenStream {
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

    let type_registration_impl = prim_meta.get_type_registration();

    let debug_impl = prim_meta.traits().gen_debug_impl();
    let hash_impl = prim_meta.traits().gen_hash_impl(reflect_crate_path);
    let partial_eq_impl = prim_meta.traits().gen_partial_eq_impl(reflect_crate_path);

    TokenStream::from(
        quote! {
            // implement GetTypeRegistration
            #type_registration_impl

            // implement Typed
            #impl_typed

            // implement Reflect
            impl #impl_generics #reflect_crate_path::Reflect for #type_name #type_generics #where_clause {
                #[inline]
                fn type_name(&self) -> &'static str {
                    ::core::any::type_name::<Self>()
                }

                #[inline]
                fn get_type_info(&self) -> &'static #reflect_crate_path::TypeInfo {
                    <Self as #reflect_crate_path::Typed>::type_info()
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
                
                #[inline]
                fn clone_value(&self) -> #QuotedBox<dyn #reflect_crate_path::Reflect> {
                    #QuotedBox::new(#QuotedClone::clone(self))
                }

                #[inline]
                fn assign(&mut self, reflected: &dyn #reflect_crate_path::Reflect) {
                    if let #QuotedOption::Some(value) = <dyn #reflect_crate_path::Reflect>::downcast_ref::<Self>(reflected) {
                        *self = #QuotedClone::clone(value);
                    } else {
                        panic!("Primitive value is not {}", ::core::any::type_name::<Self>());
                    }
                }

                fn reflect_ref<'a>(&'a self) -> #reflect_crate_path::ReflectRef<'a> {
                    #reflect_crate_path::ReflectRef::Primitive(self)
                }

                fn reflect_ref_mut<'a>(&'a mut self) -> #reflect_crate_path::ReflectRefMut<'a> {
                    #reflect_crate_path::ReflectRefMut::Primitive(self)
                }

                fn reflect_owned(self: #QuotedBox<Self>) -> #reflect_crate_path::ReflectOwned {
                    #reflect_crate_path::ReflectOwned::Primitive(self)
                }

                // implement Special Traits (Debug, Hash, PartialEq)
                #debug_impl
                #hash_impl
                #partial_eq_impl
            }
        }
    )
}