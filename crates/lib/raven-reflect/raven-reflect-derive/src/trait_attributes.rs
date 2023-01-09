use proc_macro2::{Span};
use syn::{Path, punctuated::Punctuated, NestedMeta, token::Comma, Meta, spanned::Spanned, parse::Parse};

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
    //idents: Vec<Ident>,
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
                        // // we only track reflected idents for traits not considered special
                        // _ => {
                        //     // Create the reflect ident
                        //     // We set the span to the old ident so any compile errors point to that ident instead
                        //     let mut reflect_ident = utility::get_reflect_ident(&ident_name);
                        //     reflect_ident.set_span(span);

                        //     add_unique_ident(&mut traits.idents, reflect_ident)?;
                        // }
                        _ => {}
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
            // idents: {
            //     let mut idents = self.idents;
            //     for ident in other.idents {
            //         add_unique_ident(&mut idents, ident)?;
            //     }
            //     idents
            // },
        })
    }
}

impl Parse for ReflectTraits {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let punctuated = Punctuated::<NestedMeta, Comma>::parse_terminated(input)?;
        ReflectTraits::from_nested_meta(&punctuated)
    }
}