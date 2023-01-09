use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Member, Index};

use crate::{
    reflect_meta::StructMetaInfo,
    quoted::{QuotedOption, QuotedAny, QuotedBox}
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

    let generics = struct_meta.meta().generics();

    let field_types = struct_meta.opaque_types();
    let field_count = field_idents.len();
    let field_indices = (0..field_count).collect::<Vec<usize>>();

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

    TokenStream::from(
        quote! {
            // implement Typed
            #typed_impl

            // fn test_reflect_crate_path() {
            //     println!("\nreflect crate path: {}\n", stringify!(#reflect_crate_path));
            // }

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

                fn iter(&self) -> #reflect_crate_path::type_info::FieldIter {
                    #reflect_crate_path::type_info::FieldIter::new(self)
                }
            }

            // implement Reflect
            impl #impl_generics #reflect_crate_path::Reflect for #struct_name #type_generics #where_clause {
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