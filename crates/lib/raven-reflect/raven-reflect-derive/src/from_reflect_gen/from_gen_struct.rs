use proc_macro::TokenStream;
use proc_macro2::{Span, Ident};
use quote::{ToTokens, quote};
use syn::{Member, Field, Index, Lit, LitStr, LitInt};

use crate::{
    reflect_meta::StructMetaInfo,
    quoted::{QuotedOption, QuotedDefault}, field_attributes::DefaultBehavior
};

pub(crate) fn from_gen_struct(reflected: &StructMetaInfo) -> TokenStream {
    from_gen_struct_internal(reflected, false)
}

pub(crate) fn from_gen_tuple_struct(reflected: &StructMetaInfo) -> TokenStream {
    from_gen_struct_internal(reflected, true)
}

fn from_gen_struct_internal(struct_meta: &StructMetaInfo, is_tuple: bool) -> TokenStream {
    let qoption = QuotedOption.into_token_stream();

    let struct_name = struct_meta.meta().type_name();
    let generics = struct_meta.meta().generics();
    let reflect_crate_path = struct_meta.meta().reflect_crate_path();

    let ref_struct = Ident::new("__ref_struct", Span::call_site());
    let ref_struct_type = if is_tuple {
        Ident::new("TupleStruct", Span::call_site())
    } else {
        Ident::new("Struct", Span::call_site())
    };

    let field_types = struct_meta.opaque_types();
    let MemberValuePair(active_members, active_values) =
        get_opaque_fields(struct_meta, &ref_struct, &ref_struct_type, is_tuple);

    let constructor = if struct_meta.meta().traits().contains("ReflectDefault") {
        quote!(
            let mut __this: Self = #QuotedDefault::default();
            #(
                if let #qoption::Some(__field) = #active_values() {
                    // If field exists -> use its value
                    __this.#active_members = __field;
                }
            )*
            #QuotedOption::Some(__this)
        )
    } else {
        let MemberValuePair(ignored_members, ignored_values) =
            get_ignored_fields(struct_meta, is_tuple);

        quote!(
            #QuotedOption::Some(
                Self {
                    #(#active_members: #active_values()?,)*
                    #(#ignored_members: #ignored_values,)*
                }
            )
        )
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Add FromReflect bound for each active field
    let mut where_from_reflect_clause = if where_clause.is_some() {
        quote! {#where_clause}
    } else if !active_members.is_empty() {
        quote! {where}
    } else {
        quote! {}
    };
    where_from_reflect_clause.extend(quote! {
        #(#field_types: #reflect_crate_path::FromReflect,)*
    });

    TokenStream::from(quote! {
        impl #impl_generics #reflect_crate_path::FromReflect for #struct_name #ty_generics #where_from_reflect_clause
        {
            fn from_reflect(reflect: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<Self> {
                if let #reflect_crate_path::ReflectRef::#ref_struct_type(#ref_struct) = #reflect_crate_path::Reflect::reflect_ref(reflect) {
                    #constructor
                } else {
                    #QuotedOption::None
                }
            }
        }
    })
}

/// Container for a struct's members (field name or index) and their
/// corresponding values.
struct MemberValuePair(Vec<Member>, Vec<proc_macro2::TokenStream>);

impl MemberValuePair {
    pub fn new(items: (Vec<Member>, Vec<proc_macro2::TokenStream>)) -> Self {
        Self(items.0, items.1)
    }
}

/// Get the collection of ignored field definitions
///
/// Each value of the `MemberValuePair` is a token stream that generates a
/// a default value for the ignored field.
fn get_ignored_fields(struct_meta: &StructMetaInfo, is_tuple: bool) -> MemberValuePair {
    MemberValuePair::new(
        struct_meta
            .transparent_fields()
            .map(|field| {
                let member = get_ident(field.field, field.index, is_tuple);

                let value = match &field.attrs.default_behavior {
                    DefaultBehavior::Func(path) => quote! {#path()},
                    _ => quote! {#QuotedDefault::default()},
                };

                (member, value)
            })
            .unzip(),
    )
}

/// Get the collection of active field definitions.
///
/// Each value of the `MemberValuePair` is a token stream that generates a
/// closure of type `fn() -> Option<T>` where `T` is that field's type.
fn get_opaque_fields(
    struct_meta: &StructMetaInfo,
    dyn_struct_name: &Ident,
    struct_type: &Ident,
    is_tuple: bool,
) -> MemberValuePair {
    let reflect_crate_path = struct_meta.meta().reflect_crate_path();

    MemberValuePair::new(
        struct_meta
            .opaque_fields()
            .map(|field| {
                let member = get_ident(field.field, field.index, is_tuple);
                let accessor = get_field_accessor(field.field, field.index, is_tuple);
                let ty = field.field.ty.clone();

                let get_field = quote! {
                    #reflect_crate_path::#struct_type::field(#dyn_struct_name, #accessor)
                };

                let lambda_func = match &field.attrs.default_behavior {
                    DefaultBehavior::Func(path) => quote! {
                        (||
                            if let #QuotedOption::Some(field) = #get_field {
                                <#ty as #reflect_crate_path::FromReflect>::from_reflect(field)
                            } else {
                                #QuotedOption::Some(#path())
                            }
                        )
                    },
                    DefaultBehavior::Default => quote! {
                        (||
                            if let #QuotedOption::Some(field) = #get_field {
                                <#ty as #reflect_crate_path::FromReflect>::from_reflect(field)
                            } else {
                                #QuotedOption::Some(#QuotedDefault::default())
                            }
                        )
                    },
                    DefaultBehavior::Required => quote! {
                        (|| <#ty as #reflect_crate_path::FromReflect>::from_reflect(#get_field?))
                    },
                };

                (member, lambda_func)
            })
            .unzip(),
    )
}

/// Returns the member for a given field of a struct or tuple struct.
fn get_ident(field: &Field, index: usize, is_tuple: bool) -> Member {
    if is_tuple {
        Member::Unnamed(Index::from(index))
    } else {
        field
            .ident
            .as_ref()
            .map(|ident| Member::Named(ident.clone()))
            .unwrap_or_else(|| Member::Unnamed(Index::from(index)))
    }
}

/// Returns the accessor for a given field of a struct or tuple struct.
///
/// This differs from a member in that it needs to be a number for tuple structs
/// and a string for standard structs.
fn get_field_accessor(field: &Field, index: usize, is_tuple: bool) -> Lit {
    if is_tuple {
        Lit::Int(LitInt::new(&index.to_string(), Span::call_site()))
    } else {
        field
            .ident
            .as_ref()
            .map(|ident| Lit::Str(LitStr::new(&ident.to_string(), Span::call_site())))
            .unwrap_or_else(|| Lit::Str(LitStr::new(&index.to_string(), Span::call_site())))
    }
}
