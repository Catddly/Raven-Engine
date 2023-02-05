use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Member, Index};

use crate::{
    reflect_meta::StructMetaInfo,
    quoted::{QuotedOption, QuotedBox, QuotedDefault}
};

use super::gen_typed;

/// Generate Struct, Reflect for the reflected struct.
pub(crate) fn gen_struct(struct_meta: &StructMetaInfo) -> TokenStream {
    let qoption = QuotedOption.to_token_stream();

    let reflect_crate_path = struct_meta.meta().reflect_crate_path();
    let struct_name = struct_meta.meta().type_name();

    let field_names = struct_meta.opaque_fields()
        .map(|field| {
            field
                .field
                .ident
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_else(|| field.index.to_string())
        })
        .collect::<Vec<_>>();

    let field_idents = struct_meta.opaque_fields()
        .map(|field| {
            field
                .field
                .ident
                .as_ref()
                .map(|i| Member::Named(i.clone()))
                .unwrap_or_else(|| Member::Unnamed(Index::from(field.index)))
        })
        .collect::<Vec<_>>();
        
    let field_types = struct_meta.opaque_types();
    let field_count = field_idents.len();
    let field_indices = (0..field_count).collect::<Vec<usize>>();

    let generics = struct_meta.meta().generics();

    let field_generator = quote! {
        #(#reflect_crate_path::type_info::NamedField::new::<#field_types>(#field_names) ,)*
    };
    let struct_name_string = struct_name.to_string();
    let type_info_generator = quote! {
        #reflect_crate_path::type_info::StructTypeInfo::new::<Self>(#struct_name_string, &fields)
    };

    let typed_impl = gen_typed::gen_typed(
        struct_name,
        generics,
        // this generator function only called once 
        quote! {
            let fields = [#field_generator]; // fields to be used to generate StructTypeInfo
            let info = #type_info_generator;
            #reflect_crate_path::type_info::TypeInfo::Struct(info)
        },
        reflect_crate_path,
    );

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let type_registration_impl = struct_meta.get_type_registration();

    let debug_impl = struct_meta.meta().traits().gen_debug_impl();
    let hash_impl = struct_meta.meta().traits().gen_hash_impl(reflect_crate_path);
    let partial_eq_impl = struct_meta.meta().traits()
        .gen_partial_eq_impl(reflect_crate_path)
        .unwrap_or_else(|| {
            quote! {
                fn reflect_partial_eq(&self, value: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<bool> {
                    #reflect_crate_path::special_traits::partial_eq::struct_partial_eq(self, value)
                }
            }
        });

    TokenStream::from(
        quote! {
            // implement GetTypeRegistration
            #type_registration_impl

            // implement Typed
            #typed_impl
        
            // implement Struct
            impl #impl_generics #reflect_crate_path::type_info::Struct for #struct_name #type_generics #where_clause {
                fn field(&self, name: &str) -> #QuotedOption<&dyn #reflect_crate_path::Reflect> {
                    match name {
                        #(#field_names => #qoption::Some(&self.#field_idents),)*
                        _ => #qoption::None,
                    }
                }

                fn field_mut(&mut self, name: &str) -> #QuotedOption<&mut dyn #reflect_crate_path::Reflect> {
                    match name {
                        #(#field_names => #qoption::Some(&mut self.#field_idents),)*
                        _ => #qoption::None,
                    }
                }

                fn field_at(&self, index: usize) -> #QuotedOption<&dyn #reflect_crate_path::Reflect> {
                    match index {
                        #(#field_indices => #qoption::Some(&self.#field_idents),)*
                        _ => #qoption::None,
                    }
                }

                fn field_at_mut(&mut self, index: usize) -> #QuotedOption<&mut dyn #reflect_crate_path::Reflect> {
                    match index {
                        #(#field_indices => #qoption::Some(&mut self.#field_idents),)*
                        _ => #qoption::None,
                    }
                }

                fn num_fields(&self) -> usize {
                    #field_count
                }

                fn field_name_at(&self, index: usize) -> #QuotedOption<&str> {
                    match index {
                        #(#field_indices => #qoption::Some(#field_names),)*
                        _ => #qoption::None,
                    }
                }

                fn iter(&self) -> #reflect_crate_path::type_info::StructFieldIter {
                    #reflect_crate_path::type_info::StructFieldIter::new(self)
                }

                fn clone_dynamic(&self) -> #reflect_crate_path::type_info::DynamicStruct {
                    let mut dynamic: #reflect_crate_path::type_info::DynamicStruct = #QuotedDefault::default();
                    dynamic.set_name(::std::string::ToString::to_string(#reflect_crate_path::Reflect::type_name(self)));
                    #(dynamic.add_field_boxed(#field_names, #reflect_crate_path::Reflect::clone_value(&self.#field_idents));)*
                    dynamic
                }
            }

            // implement Reflect
            impl #impl_generics #reflect_crate_path::Reflect for #struct_name #type_generics #where_clause {
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
                    #QuotedBox::new(#reflect_crate_path::type_info::Struct::clone_dynamic(self))
                }

                #[inline]
                fn assign(&mut self, reflected: &dyn #reflect_crate_path::Reflect) {
                    if let #reflect_crate_path::ReflectRef::Struct(struct_value) = #reflect_crate_path::Reflect::reflect_ref(reflected) {
                        for (i, value) in ::core::iter::Iterator::enumerate(#reflect_crate_path::type_info::Struct::iter(struct_value)) {
                            let name = #reflect_crate_path::type_info::Struct::field_name_at(struct_value, i).unwrap();
                            #reflect_crate_path::type_info::Struct::field_mut(self, name).map(|v| v.assign(value));
                        }
                    } else {
                        panic!("Attempted to apply non-struct type to struct type.");
                    }
                }

                fn reflect_ref<'a>(&'a self) -> #reflect_crate_path::ReflectRef<'a> {
                    #reflect_crate_path::ReflectRef::Struct(self)
                }

                fn reflect_ref_mut<'a>(&'a mut self) -> #reflect_crate_path::ReflectRefMut<'a> {
                    #reflect_crate_path::ReflectRefMut::Struct(self)
                }

                fn reflect_owned(self: #QuotedBox<Self>) -> #reflect_crate_path::ReflectOwned {
                    #reflect_crate_path::ReflectOwned::Struct(self)
                }

                // implement Special Traits (Debug, Hash, PartialEq)
                #debug_impl
                #hash_impl
                #partial_eq_impl
            }
        }
    )
}