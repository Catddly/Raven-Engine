use proc_macro2::{Span, Ident, TokenStream};
use quote::quote_spanned;
use syn::{Path, punctuated::Punctuated, NestedMeta, token::Comma, Meta, spanned::Spanned, parse::Parse};

use crate::quoted::{QuotedAny, QuotedOption, QuotedDefault};

const DEBUG_TRAIT: &str = "Debug";
const HASH_TRAIT: &str = "Hash";
const PARTIAL_EQ_TRAIT: &str = "PartialEq";

#[derive(Clone, Default)]
pub(crate) enum TraitImplStatus {
    /// The trait is `NOT` registered as implemented.
    #[default]
    NotImplemented,
    /// The trait is registered as implemented.
    Implemented(Span),
    /// The trait is registered with custom function to replace the derive behavior.
    CustomImpl(Path, Span),
}

impl TraitImplStatus {
    /// Merges this [`TraitImplStatus`] with another.
    ///
    /// Returns whichever value is not [`TraitImplStatus::NotImplemented`].
    /// If both values are [`TraitImplStatus::NotImplemented`], then that is returned.
    /// Otherwise, an error is returned if neither value is [`TraitImplStatus::NotImplemented`].
    pub fn merge(self, other: TraitImplStatus) -> anyhow::Result<TraitImplStatus, syn::Error> {
        match (self, other) {
            (TraitImplStatus::NotImplemented, value) | (value, TraitImplStatus::NotImplemented) => Ok(value),
            (_, TraitImplStatus::Implemented(span) | TraitImplStatus::CustomImpl(_, span)) => {
                Err(syn::Error::new(span, "Conflicting type data registration"))
            }
        }
    }
}

/// A collection of traits that have been registered for a reflected type.
/// 
/// Some traits are utilized to be used in reflection.
/// 'Reflect' derive macro using helper attributes #[reflect()] to keep track of it.
/// 
/// 
#[derive(Default, Clone)]
pub(crate) struct ReflectTraits {
    debug_impl: TraitImplStatus,
    hash_impl: TraitImplStatus,
    partial_eq_impl: TraitImplStatus,
    idents: Vec<Ident>,
}

impl ReflectTraits {
    /// Accept a comma punctuated traits sequence.
    /// (e.g. Debug, Hash, PartialEq, ) 
    pub(crate) fn from_nested_meta(
        nested_metas: &Punctuated<NestedMeta, Comma>,
    ) -> anyhow::Result<Self, syn::Error> {
        let mut traits = ReflectTraits::default();

        for nested_meta in nested_metas.iter() {
            match nested_meta {
                // handle #[derive(Clone, Copy)]
                // here path is `Clone`
                NestedMeta::Meta(Meta::Path(path)) => {
                    // get the first ident in the path (hopefully the path only contains one and not `std::hash::Hash`)
                    let Some(segment) = path.segments.iter().next() else {
                        continue;
                    };

                    let ident = &segment.ident;
                    let ident_name = ident.to_string();

                    // track the span where the trait is implemented for future errors
                    let span = ident.span();

                    match ident_name.as_str() {
                        DEBUG_TRAIT => {
                            traits.debug_impl = traits.debug_impl.merge(TraitImplStatus::Implemented(span))?;
                        }
                        HASH_TRAIT => {
                            traits.hash_impl = traits.hash_impl.merge(TraitImplStatus::Implemented(span))?;
                        }
                        PARTIAL_EQ_TRAIT => {
                            traits.partial_eq_impl = traits.partial_eq_impl.merge(TraitImplStatus::Implemented(span))?;
                        }
                        // we only track reflected idents for traits not considered special
                        _ => {
                            // Create the reflect ident
                            // We set the span to the old ident so any compile errors point to that ident instead
                            let mut reflect_ident = get_reflect_ident(&ident_name);
                            reflect_ident.set_span(span);

                            add_unique_ident(&mut traits.idents, reflect_ident)?;
                        }
                    }
                }
                // handle #[derive(Hash(custom_hash_func))]
                NestedMeta::Meta(Meta::List(list)) => {
                    // get the first ident in the path (hopefully the path only contains one and not `std::hash::Hash`)
                    let Some(segment) = list.path.segments.iter().next() else {
                        continue;
                    };

                    let ident = segment.ident.to_string();

                    // Track the span where the trait is implemented for future errors
                    let span = ident.span();

                    let list_meta = list.nested.iter().next();
                    // first literal of the list
                    if let Some(NestedMeta::Meta(Meta::Path(path))) = list_meta {
                        // this should be the path of the custom function
                        let trait_func_ident = TraitImplStatus::CustomImpl(path.clone(), span);

                        match ident.as_str() {
                            DEBUG_TRAIT => {
                                traits.debug_impl = traits.debug_impl.merge(trait_func_ident)?;
                            }
                            HASH_TRAIT => {
                                traits.hash_impl = traits.hash_impl.merge(trait_func_ident)?;
                            }
                            PARTIAL_EQ_TRAIT => {
                                traits.partial_eq_impl = traits.partial_eq_impl.merge(trait_func_ident)?;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(traits)
    }

    pub fn merge(self, other: ReflectTraits) -> anyhow::Result<Self, syn::Error> {
        Ok(ReflectTraits {
            debug_impl: self.debug_impl.merge(other.debug_impl)?,
            hash_impl: self.hash_impl.merge(other.hash_impl)?,
            partial_eq_impl: self.partial_eq_impl.merge(other.partial_eq_impl)?,
            idents: {
                let mut idents = self.idents;
                for ident in other.idents {
                    add_unique_ident(&mut idents, ident)?;
                }
                idents
            },
        })
    }

    /// Returns true if the given reflected trait name (i.e. `ReflectDefault` for `Default`)
    /// is registered for this type.
    pub fn contains(&self, name: &str) -> bool {
        self.idents.iter().any(|ident| ident == name)
    }

    /// The list of reflected traits by their reflected ident (i.e. `ReflectDefault` for `Default`).
    pub fn idents(&self) -> &[Ident] {
        &self.idents
    }

    /// Generate implementation for special trait Debug.
    pub fn gen_debug_impl(&self) -> Option<TokenStream> {
        match &self.debug_impl {
            &TraitImplStatus::Implemented(span) => Some({quote_spanned! {span=>
                fn debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    ::core::fmt::Debug::fmt(self, f)
                }
            }}),
            &TraitImplStatus::CustomImpl(ref custom_impl, span) => Some({quote_spanned! {span=>
                fn debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    // forward to custom impl function
                    #custom_impl(self, f)
                }
            }}),
            TraitImplStatus::NotImplemented => None,
        }
    }

    /// Generate implementation for special trait Hash.
    /// 
    /// Use reflect_hash to avoid name collision with hash.
    pub fn gen_hash_impl(&self, reflect_crate_path: &Path) -> Option<TokenStream> {
        match &self.hash_impl {
            &TraitImplStatus::Implemented(span) => Some(quote_spanned! {span=>
                fn reflect_hash(&self) -> #QuotedOption<u64> {
                    use ::core::hash::{Hash, Hasher};
                    let mut hasher: #reflect_crate_path::ReflectHasher = #QuotedDefault::default();
                    Hash::hash(&#QuotedAny::type_id(self), &mut hasher);
                    Hash::hash(self, &mut hasher);
                    #QuotedOption::Some(Hasher::finish(&hasher))
                }
            }),
            &TraitImplStatus::CustomImpl(ref custom_impl, span) => Some(quote_spanned! {span=>
                fn reflect_hash(&self) -> #QuotedOption<u64> {
                    // forward to custom impl function
                    #QuotedOption::Some(#custom_impl(self))
                }
            }),
            TraitImplStatus::NotImplemented => None,
        }
    }

    /// Generate implementation for special trait PartialEq.
    /// 
    /// Use reflect_partial_eq to avoid name collision with partial_eq.
    pub fn gen_partial_eq_impl(&self, reflect_crate_path: &Path) -> Option<TokenStream> {
        match &self.partial_eq_impl {
            &TraitImplStatus::Implemented(span) => Some(quote_spanned! {span=>
                fn reflect_partial_eq(&self, value: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<bool> {
                    let value = <dyn #reflect_crate_path::Reflect>::as_any(value);
                    if let #QuotedOption::Some(value) = <dyn #QuotedAny>::downcast_ref::<Self>(value) {
                        #QuotedOption::Some(::core::cmp::PartialEq::eq(self, value))
                    } else {
                        #QuotedOption::Some(false)
                    }
                }
            }),
            &TraitImplStatus::CustomImpl(ref custom_impl, span) => Some(quote_spanned! {span=>
                fn reflect_partial_eq(&self, value: &dyn #reflect_crate_path::Reflect) -> #QuotedOption<bool> {
                    #QuotedOption::Some(#custom_impl(self, value))
                }
            }),
            TraitImplStatus::NotImplemented => None,
        }
    }
}

impl Parse for ReflectTraits {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let punctuated = Punctuated::<NestedMeta, Comma>::parse_terminated(input)?;
        ReflectTraits::from_nested_meta(&punctuated)
    }
}

/// Returns the "reflected" ident for a given string.
///
/// # Example
///
/// ```ignore
/// let reflected: Ident = get_reflect_ident("Hash");
/// assert_eq!("ReflectHash", reflected.to_string());
/// ```
pub(crate) fn get_reflect_ident(name: &str) -> Ident {
    let reflected = format!("Reflect{name}");
    Ident::new(&reflected, Span::call_site())
}

/// Adds an identifier to a vector of identifiers if it is not already present.
///
/// Returns an error if the identifier already exists in the list.
fn add_unique_ident(idents: &mut Vec<Ident>, ident: Ident) -> Result<(), syn::Error> {
    let ident_name = ident.to_string();
    if idents.iter().any(|i| i == ident_name.as_str()) {
        return Err(syn::Error::new(ident.span(), "Conflict type data registration!"));
    }

    idents.push(ident);
    Ok(())
}