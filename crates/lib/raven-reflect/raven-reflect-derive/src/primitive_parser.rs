use proc_macro2::Ident;
use syn::{Generics, Attribute, parse::Parse, token, parenthesized, WhereClause};

use crate::trait_attributes::ReflectTraits;

/// Parse input as: 
/// 
/// ```ignore
/// // With traits
/// 1. u32(Debug, CustomTraits...)
/// // With type generic
/// 2. u32<T1, T2...>(Debug, CustomTraits...)
/// // With where clause
/// 3. u32<<T1, T2...> where T1: OtherTrait (Debug, CustomTraits...)
/// ```
pub(crate) struct PrimitiveParser {
    #[allow(dead_code)]
    pub attrs: Vec<Attribute>,
    pub type_name: Ident,
    pub generics: Generics,
    pub traits: Option<ReflectTraits>,
}

impl Parse for PrimitiveParser {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // parse outer attributes
        let attrs = input.call(Attribute::parse_outer)?;
        // parse type name as identifier first
        let type_name = input.parse::<Ident>()?;
        // then we parse type generics
        let generics = input.parse::<Generics>()?;

        // peek ahead to check where clause
        let mut where_clause = None;
        let mut lookahead = input.lookahead1();
        if lookahead.peek(token::Where) {
            where_clause = Some(input.parse::<WhereClause>()?);
            lookahead = input.lookahead1();
        }

        let mut traits = None;
        // parse traits
        if lookahead.peek(token::Paren) {
            let content;
            parenthesized!(content in input);
            traits = Some(content.parse::<ReflectTraits>()?);
        }

        Ok(Self {
            attrs,
            type_name,
            generics: Generics {
                where_clause,
                ..generics
            },
            traits,
        })
    }
}