use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{ToTokens, quote};
use syn::{Fields, Member};

use crate::{
    reflect_meta::{EnumMetaInfo, EnumVariantFields, StructField, EnumVariant},
    quoted::{QuotedOption, QuotedBox, QuotedDefault}
};

use super::gen_typed;

pub(crate) fn gen_enum(enum_meta: &EnumMetaInfo) -> TokenStream {
    let reflect_crate_path = enum_meta.meta().reflect_crate_path();
    let enum_name = enum_meta.meta().type_name();

    // common used parameters in Enum trait
    let ref_name = Ident::new("__name_param", Span::call_site());
    let ref_index = Ident::new("__index_param", Span::call_site());
    let ref_value = Ident::new("__value_param", Span::call_site());

    let EnumImpls {
        variant_info,
        enum_field,
        enum_field_at,
        enum_index_of,
        enum_name_at,
        enum_field_len,
        enum_variant_name,
        enum_variant_index,
        enum_variant_form,
    } = generate_impls(&enum_meta, &ref_index, &ref_name);

    let EnumVariantConstructors {
        variant_names,
        variant_constructors,
    } = get_variant_constructors(&enum_meta, &ref_value, true);

    let enum_string_name = enum_name.to_string();
    let type_info_generator = {
        quote! {
            #reflect_crate_path::type_info::EnumTypeInfo::new::<Self>(#enum_string_name, &variants)
        }
    };

    let generics = enum_meta.meta().generics();

    let typed_impl = gen_typed::gen_typed(
        enum_name,
        generics,
        // this generator function only called once 
        quote! {
            let variants = [#(#variant_info),*]; // variants to be used to generate EnumTypeInfo
            let info = #type_info_generator;
            #reflect_crate_path::type_info::TypeInfo::Enum(info)
        },
        reflect_crate_path,
    );

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let type_registration_impl = enum_meta.meta().get_type_registration();

    let debug_impl = enum_meta.meta().traits().gen_debug_impl();
    let hash_impl = enum_meta.meta().traits().gen_hash_impl(reflect_crate_path);
    let partial_eq_impl = enum_meta.meta().traits().gen_partial_eq_impl(reflect_crate_path);
    
    TokenStream::from(
        quote! {
            // implement GetTypeRegistration
            #type_registration_impl

            // implement Typed
            #typed_impl

            // implement Enum
            impl #impl_generics #reflect_crate_path::type_info::Enum for #enum_name #type_generics #where_clause {
                fn field(&self, #ref_name: &str) -> #QuotedOption<&dyn #reflect_crate_path::Reflect> {
                    match self {
                        #(#enum_field,)*
                        _ => #QuotedOption::None,
                    }
                }

                fn field_mut(&mut self, #ref_name: &str) -> #QuotedOption<&mut dyn #reflect_crate_path::Reflect> {
                    match self {
                        #(#enum_field,)*
                        _ => #QuotedOption::None,
                    }
                }
                
                fn field_at(&self, #ref_index: usize) -> #QuotedOption<&dyn #reflect_crate_path::Reflect> {
                    match self {
                        #(#enum_field_at,)*
                        _ => #QuotedOption::None,
                    }
                }

                fn field_at_mut(&mut self, #ref_index: usize) -> #QuotedOption<&mut dyn #reflect_crate_path::Reflect> {
                    match self {
                        #(#enum_field_at,)*
                        _ => #QuotedOption::None,
                    }
                }
            
                fn index_of(&self, #ref_name: &str) -> #QuotedOption<usize> {
                    match self {
                        #(#enum_index_of,)*
                        _ => #QuotedOption::None,
                    }
                }
            
                fn field_name_at(&self, #ref_index: usize) -> #QuotedOption<&str> {
                    match self {
                        #(#enum_name_at,)*
                        _ => #QuotedOption::None,
                    }
                }
            
                fn iter(&self) -> #reflect_crate_path::type_info::VariantFieldIter {
                    #reflect_crate_path::type_info::VariantFieldIter::new(self)
                }
            
                #[inline]
                fn num_fields(&self) -> usize {
                    match self {
                        #(#enum_field_len,)*
                        _ => 0,
                    }
                }
                
                #[inline]
                fn variant_name(&self) -> &str {
                    match self {
                        #(#enum_variant_name,)*
                        _ => unreachable!(),
                    }
                }

                #[inline]
                fn variant_index(&self) -> usize {
                    match self {
                        #(#enum_variant_index,)*
                        _ => unreachable!(),
                    }
                }

                #[inline]
                fn variant_form(&self) -> #reflect_crate_path::VariantForm {
                    match self {
                        #(#enum_variant_form,)*
                        _ => unreachable!(),
                    }
                }

                #[inline]
                fn clone_dynamic(&self) -> #reflect_crate_path::type_info::DynamicEnum {
                    #reflect_crate_path::DynamicEnum::from_ref::<Self>(self)
                }
            }
        
            // implement Reflect
            impl #impl_generics #reflect_crate_path::Reflect for #enum_name #type_generics #where_clause {
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
                    #QuotedBox::new(#reflect_crate_path::type_info::Enum::clone_dynamic(self))
                }

                #[inline]
                fn assign(&mut self, #ref_value: &dyn #reflect_crate_path::Reflect) {
                    if let #reflect_crate_path::ReflectRef::Enum(#ref_value) = #reflect_crate_path::Reflect::reflect_ref(#ref_value) {
                        if #reflect_crate_path::Enum::variant_name(self) == #reflect_crate_path::Enum::variant_name(#ref_value) {
                            // same variant -> just update fields
                            match #reflect_crate_path::Enum::variant_form(#ref_value) {
                                #reflect_crate_path::VariantForm::Struct => {
                                    for field in #reflect_crate_path::Enum::iter(#ref_value) {
                                        let name = field.name().unwrap();
                                        #reflect_crate_path::Enum::field_mut(self, name).map(|v| v.assign(field.value()));
                                    }
                                }
                                #reflect_crate_path::VariantForm::Tuple => {
                                    for (index, field) in ::core::iter::Iterator::enumerate(#reflect_crate_path::Enum::iter(#ref_value)) {
                                        #reflect_crate_path::Enum::field_at_mut(self, index).map(|v| v.assign(field.value()));
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // different variant -> perform a switch
                            match #reflect_crate_path::Enum::variant_name(#ref_value) {
                                #(#variant_names => {
                                    *self = #variant_constructors
                                })*
                                name => panic!("Variant with name `{}` does not exist on enum `{}`", name, ::core::any::type_name::<Self>()),
                            }
                        }
                    } else {
                        panic!("`{}` is not an enum", #reflect_crate_path::Reflect::type_name(#ref_value));
                    }
                }

                fn reflect_ref<'a>(&'a self) -> #reflect_crate_path::ReflectRef<'a> {
                    #reflect_crate_path::ReflectRef::Enum(self)
                }

                fn reflect_ref_mut<'a>(&'a mut self) -> #reflect_crate_path::ReflectRefMut<'a> {
                    #reflect_crate_path::ReflectRefMut::Enum(self)
                }

                fn reflect_owned(self: #QuotedBox<Self>) -> #reflect_crate_path::ReflectOwned {
                    #reflect_crate_path::ReflectOwned::Enum(self)
                }

                // implement Special Traits (Debug, Hash, PartialEq)
                #debug_impl
                #hash_impl
                #partial_eq_impl
            }
        }
    )
}

struct EnumImpls {
    variant_info: Vec<proc_macro2::TokenStream>,
    enum_field: Vec<proc_macro2::TokenStream>,
    enum_field_at: Vec<proc_macro2::TokenStream>,
    enum_index_of: Vec<proc_macro2::TokenStream>,
    enum_name_at: Vec<proc_macro2::TokenStream>,
    enum_field_len: Vec<proc_macro2::TokenStream>,
    enum_variant_name: Vec<proc_macro2::TokenStream>,
    enum_variant_index: Vec<proc_macro2::TokenStream>,
    enum_variant_form: Vec<proc_macro2::TokenStream>,
}

fn generate_impls(reflect_enum: &EnumMetaInfo, ref_index: &Ident, ref_name: &Ident) -> EnumImpls {
    let reflect_crate_path = reflect_enum.meta().reflect_crate_path();

    let mut variant_info = Vec::new();
    let mut enum_field = Vec::new();
    let mut enum_field_at = Vec::new();
    let mut enum_index_of = Vec::new();
    let mut enum_name_at = Vec::new();
    let mut enum_field_len = Vec::new();
    let mut enum_variant_name = Vec::new();
    let mut enum_variant_index = Vec::new();
    let mut enum_variant_form = Vec::new();
    
    for (variant_index, variant) in reflect_enum.variants().iter().enumerate() {
        let ident = &variant.variant.ident;
        let name = ident.to_string();
        let unit = reflect_enum.get_enum_unit(ident);

        let variant_form_ident = match variant.variant.fields {
            Fields::Unit => Ident::new("Unit", Span::call_site()),
            Fields::Unnamed(..) => Ident::new("Tuple", Span::call_site()),
            Fields::Named(..) => Ident::new("Struct", Span::call_site()),
        };

        let variant_info_ident = match variant.variant.fields {
            Fields::Unit => Ident::new("UnitVariantInfo", Span::call_site()),
            Fields::Unnamed(..) => Ident::new("TupleVariantInfo", Span::call_site()),
            Fields::Named(..) => Ident::new("StructVariantInfo", Span::call_site()),
        };

        enum_variant_name.push(quote! {
            #unit{..} => #name
        });
        enum_variant_index.push(quote! {
            #unit{..} => #variant_index
        });

        fn get_field_args(
            fields: &[StructField],
            mut generate_for_field: impl FnMut(usize, usize, &StructField) -> proc_macro2::TokenStream,
        ) -> Vec<proc_macro2::TokenStream> {
            let mut constructor_argument = Vec::new();
            let mut reflect_idx = 0;

            for field in fields.iter() {
                if field.attrs.ignore_behavior.is_transparent() {
                    // ignore
                    continue;
                }

                constructor_argument.push(generate_for_field(reflect_idx, field.index, field));
                reflect_idx += 1;
            }
            
            constructor_argument
        }

        let mut push_variant_func = |_variant: &EnumVariant, arguments: proc_macro2::TokenStream, field_len: usize| {
                // generate VariantInfo in [`raven-reflect::type_info::types::enum::variant::VariantInfo`]
                variant_info.push(quote! {
                    #reflect_crate_path::VariantInfo::#variant_form_ident(
                        #reflect_crate_path::#variant_info_ident::new(#arguments)
                    )
                });
                enum_field_len.push(quote! {
                    #unit{..} => #field_len
                });
                enum_variant_form.push(quote! {
                    #unit{..} => #reflect_crate_path::VariantForm::#variant_form_ident
                });
            };

        match &variant.fields {
            EnumVariantFields::Unit => {
                push_variant_func(variant, quote!(#name), 0);
            }
            EnumVariantFields::Unnamed(fields) => {
                let args = get_field_args(fields, |reflect_idx, declaration_index, field| {
                    let declare_field = syn::Index::from(declaration_index);

                    enum_field_at.push(quote! {
                        #unit { #declare_field : value, .. } if #ref_index == #reflect_idx => #QuotedOption::Some(value)
                    });

                    let field_ty = &field.field.ty;
                    quote! {
                        #reflect_crate_path::UnnamedField::new::<#field_ty>(#reflect_idx)
                    }
                });

                let field_len = args.len();
                push_variant_func(variant, quote!(#name, &[ #(#args),* ]), field_len);
            }
            EnumVariantFields::Named(fields) => {
                let args = get_field_args(fields, |reflect_idx, _, field| {
                    let field_ident = field.field.ident.as_ref().unwrap();
                    let field_name = field_ident.to_string();

                    enum_field.push(quote! {
                        #unit{ #field_ident, .. } if #ref_name == #field_name => #QuotedOption::Some(#field_ident)
                    });
                    enum_field_at.push(quote! {
                        #unit{ #field_ident, .. } if #ref_index == #reflect_idx => #QuotedOption::Some(#field_ident)
                    });
                    enum_index_of.push(quote! {
                        #unit{ .. } if #ref_name == #field_name => #QuotedOption::Some(#reflect_idx)
                    });
                    enum_name_at.push(quote! {
                        #unit{ .. } if #ref_index == #reflect_idx => #QuotedOption::Some(#field_name)
                    });

                    let field_ty = &field.field.ty;
                    quote! {
                        #reflect_crate_path::NamedField::new::<#field_ty>(#field_name)
                    }
                });

                let field_len = args.len();
                push_variant_func(variant, quote!(#name, &[ #(#args),* ]), field_len);
            }
        };
    }

    EnumImpls {
        variant_info,
        enum_field,
        enum_field_at,
        enum_index_of,
        enum_name_at,
        enum_field_len,
        enum_variant_name,
        enum_variant_index,
        enum_variant_form,
    }
}

pub(crate) struct EnumVariantConstructors {
    /// The names of each variant as a string.
    pub variant_names: Vec<String>,
    /// The stream of tokens that will construct each variant.
    pub variant_constructors: Vec<proc_macro2::TokenStream>,
}

/// Gets the constructors for all variants in the given enum.
pub(crate) fn get_variant_constructors(
    reflect_enum: &EnumMetaInfo,
    ref_value: &Ident,
    can_panic: bool,
) -> EnumVariantConstructors {
    let reflect_crate_path = reflect_enum.meta().reflect_crate_path();
    let num_variant = reflect_enum.variants().len();

    let mut variant_names = Vec::with_capacity(num_variant);
    let mut variant_constructors = Vec::with_capacity(num_variant);

    for variant in reflect_enum.variants() {
        let ident = &variant.variant.ident;
        let name = ident.to_string();
        let variant_constructor = reflect_enum.get_enum_unit(ident);

        let fields = match &variant.fields {
            EnumVariantFields::Unit => &[],
            EnumVariantFields::Named(fields) | EnumVariantFields::Unnamed(fields) => {
                fields.as_slice()
            }
        };

        let mut reflect_index: usize = 0;
        let constructor_fields = fields
            .iter()
            .enumerate()
            .map(|(declar_index, field)| {
                let field_ident = ident_or_index(field.field.ident.as_ref(), declar_index);
                let field_value = if field.attrs.ignore_behavior.is_transparent() {
                    quote! { #QuotedDefault::default() }
                } else {
                    let error_repr = field.field.ident.as_ref().map_or_else(
                        || format!("at index {reflect_index}"),
                        |name| format!("`{name}`"),
                    );
                    let unwrapper = if can_panic {
                        let type_err_message = format!(
                            "the field {error_repr} should be of type `{}`",
                            field.field.ty.to_token_stream()
                        );
                        quote!(.expect(#type_err_message))
                    } else {
                        quote!(?)
                    };
                    let field_accessor = match &field.field.ident {
                        Some(ident) => {
                            let name = ident.to_string();
                            quote!(.field(#name))
                        }
                        None => quote!(.field_at(#reflect_index)),
                    };
                    reflect_index += 1;
                    let missing_field_err_message = format!("the field {error_repr} was not declared");
                    let accessor = quote!(#field_accessor .expect(#missing_field_err_message));

                    quote! {
                        #reflect_crate_path::FromReflect::from_reflect(#ref_value #accessor)
                        #unwrapper
                    }
                };
                quote! { #field_ident : #field_value }
            });

        variant_constructors.push(quote! {
            #variant_constructor { #( #constructor_fields ),* }
        });
        variant_names.push(name);
    }

    EnumVariantConstructors {
        variant_names,
        variant_constructors,
    }
}

fn ident_or_index(ident: Option<&Ident>, index: usize) -> Member {
    ident.map_or_else(
        || Member::Unnamed(index.into()),
        |ident| Member::Named(ident.clone()),
    )
}