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