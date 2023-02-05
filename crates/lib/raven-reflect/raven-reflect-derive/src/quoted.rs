//! From [`bevy_reflect_derive::fq_std`].
//! 
//! This module contains unit struct as variable to span while using `quote!()` or `spanned_quote!()`.
//! To create hygienic proc macros, all the names must be its fully qualified form.
//! These unit structs help us to not specify the fully qualified name every single time.
//! 

use quote::{ToTokens, quote};

pub(crate) struct QuotedOption;

pub(crate) struct QuotedAny;

pub(crate) struct QuotedBox;

pub(crate) struct QuotedDefault;

pub(crate) struct QuotedClone;

pub(crate) struct QuotedResult;

impl ToTokens for QuotedOption {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote!(::core::option::Option).to_tokens(tokens)
    }
}

impl ToTokens for QuotedAny {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote!(::core::any::Any).to_tokens(tokens)
    }
}

impl ToTokens for QuotedBox {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote!(::std::boxed::Box).to_tokens(tokens)
    }
}

impl ToTokens for QuotedDefault {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote!(::core::default::Default).to_tokens(tokens)
    }
}

impl ToTokens for QuotedClone {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote!(::core::clone::Clone).to_tokens(tokens)
    }
}

impl ToTokens for QuotedResult {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote!(::std::result::Result).to_tokens(tokens)
    }
}