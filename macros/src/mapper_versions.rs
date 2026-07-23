use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

use proc_macro2::{Ident, TokenStream};
use quote::{quote, IdentFragment, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    braced, parse2, Expr, GenericParam, Generics, ItemStruct, LitInt, Token,
    TypeParamBound, WherePredicate,
};

pub fn mapper_versions(attribute_args: &TokenStream, item: &TokenStream) -> TokenStream {
    let parsed_struct: ItemStruct = match parse2(item.clone()) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error(),
    };

    let struct_ident = parsed_struct.ident.clone();

    let (variant1_path, variant2_path) = match parse_generics(&parsed_struct) {
        Ok(v) => v,
        Err(err) => return err.to_compile_error(),
    };

    let delegation = quote! {
        #variant1_path!(#variant2_path, #struct_ident, #attribute_args);
    };

    quote! {
        #item
        #delegation
    }
}

const GENERICS_ERROR_MESSAGE: &str = "`#[mapper_versions]` could not determine the mapper variant \
                                      traits.
The annotated struct must declare two generic type parameters bounded by a trait annotated with
`#[mapper_register]`, for example:
    struct MMC1Common<V: MMC1Variant, S: MMC1Submapper> { ... }
where
    #[monsoon_macro::mapper_variant]
    trait MMC1Variant {
        ...
    }

    #[monsoon_macro::mapper_variant]
    trait MMC1Submapper {
        ...
    }";

fn parse_generics(parsed_struct: &ItemStruct) -> Result<(Ident, Ident), syn::Error> {
    if parsed_struct.generics.params.len() < 2 {
        return Err(syn::Error::new_spanned(
            &parsed_struct.ident,
            GENERICS_ERROR_MESSAGE,
        ));
    }

    let valid_variant_bounds = get_valid_variant_bounds(&parsed_struct.generics);

    if valid_variant_bounds.len() < 2 {
        return Err(syn::Error::new_spanned(
            &parsed_struct.ident,
            GENERICS_ERROR_MESSAGE,
        ));
    }

    Ok((
        valid_variant_bounds[0].clone(),
        valid_variant_bounds[1].clone(),
    ))
}

fn get_valid_variant_bounds(generics: &Generics) -> Vec<Ident> {
    let mut res = Vec::new();

    for param in &generics.params {
        if let GenericParam::Type(t) = param {
            let bounds: Vec<_> = t
                .bounds
                .iter()
                .filter_map(|b| match b {
                    TypeParamBound::Trait(t) => Some(t.path.get_ident()),
                    _ => None,
                })
                .flatten()
                .collect();

            if bounds.is_empty()
                && let Some(where_clause) = &generics.where_clause
            {
                let where_bounds: Vec<_> = where_clause
                    .predicates
                    .iter()
                    .filter_map(|w| match w {
                        WherePredicate::Type(t) => Some(
                            t.bounds
                                .iter()
                                .filter_map(|b| match b {
                                    TypeParamBound::Trait(t) => Some(t.path.get_ident()),
                                    _ => None,
                                })
                                .flatten(),
                        ),
                        _ => None,
                    })
                    .flatten()
                    .collect();

                if !where_bounds.is_empty() {
                    res.push(where_bounds[0].clone());
                }
            } else {
                res.push(bounds[0].clone());
            }
        }
    }

    res
}

mod kw {
    syn::custom_keyword!(revisions);
    syn::custom_keyword!(submappers);
    syn::custom_keyword!(mapper);
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct MapperVersionsArgs {
    pub enum_to_generate: EnumToGenerate,
    pub revisions: Variants<kw::revisions>,
    pub submappers: Variants<kw::submappers>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct EnumToGenerate(pub Ident);

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Variants<V: Parse>(pub Vec<Variant>, PhantomData<V>);

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Variant {
    pub name: VariantIdentifier,
    pub mapper: MapperDefinition,
    pub fields: Vec<TraitAssignment>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct MapperDefinition(pub Option<Ident>);

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TraitAssignment {
    Implicit(ImplicitTraitAssignment),
    Explicit(ExplicitTraitAssignment),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ImplicitTraitAssignment {
    pub value: Expr,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ExplicitTraitAssignment {
    pub ident: Ident,
    pub value: Expr,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum VariantIdentifier {
    Ident(Ident),
    Num(LitInt),
}
impl IdentFragment for VariantIdentifier {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            VariantIdentifier::Ident(i) => IdentFragment::fmt(i, f),
            VariantIdentifier::Num(n) => IdentFragment::fmt(n.base10_digits(), f),
        }
    }
}
impl Display for VariantIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VariantIdentifier::Ident(i) => {
                write!(f, "{i}")
            }
            VariantIdentifier::Num(n) => {
                write!(f, "{n}")
            }
        }
    }
}

impl Parse for MapperDefinition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<kw::mapper>()?;
        let _ = input.parse::<Token![=]>()?;
        let path = input.parse::<Ident>()?;

        Ok(Self(Some(path)))
    }
}

impl Parse for VariantIdentifier {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(LitInt) {
            Ok(Self::Num(input.parse::<LitInt>()?))
        } else {
            Ok(Self::Ident(input.parse::<Ident>()?))
        }
    }
}

impl Parse for MapperVersionsArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let enum_to_generate = input.parse::<EnumToGenerate>()?;
        let _ = input.parse::<Token![,]>()?;
        let revisions = input.parse::<Variants<kw::revisions>>()?;
        let _ = input.parse::<Token![,]>()?;
        let submappers = input.parse::<Variants<kw::submappers>>()?;

        Ok(Self {
            enum_to_generate,
            revisions,
            submappers,
        })
    }
}

impl<V: Parse> Parse for Variants<V> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<V>()?;

        let content;
        braced!(content in input);

        let variants = Punctuated::<Variant, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();

        Ok(Self(variants, PhantomData))
    }
}

impl Parse for Variant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<VariantIdentifier>()?;
        let mut mapper = MapperDefinition(None);
        let mut fields = Vec::new();

        if input.peek(Token![=>]) {
            let _ = input.parse::<Token![=>]>()?;

            let assignments: Punctuated<TraitAssignment, Token![,]> =
                if input.peek(syn::token::Brace) {
                    let content;
                    braced!(content in input);
                    Punctuated::parse_terminated(&content)?
                } else {
                    let mut p = Punctuated::new();
                    p.push_value(input.parse()?);
                    p
                };

            for assignment in assignments {
                match assignment {
                    TraitAssignment::Explicit(ExplicitTraitAssignment {
                        ident,
                        value,
                    }) if ident == "mapper" => {
                        let val = &value;
                        mapper = parse2(quote! {
                            mapper = #val
                        })?;
                    }
                    _ => {
                        fields.push(assignment);
                    }
                }
            }
        }

        Ok(Self {
            name,
            mapper,
            fields,
        })
    }
}

impl Parse for ExplicitTraitAssignment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;
        let _ = input.parse::<Token![=]>()?;
        let value = input.parse::<Expr>()?;

        Ok(Self {
            ident,
            value,
        })
    }
}

impl Parse for ImplicitTraitAssignment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let value = input.parse::<Expr>()?;

        Ok(Self {
            value,
        })
    }
}

impl Parse for TraitAssignment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek2(Token![=]) {
            let exp = input.parse::<ExplicitTraitAssignment>()?;
            Ok(TraitAssignment::Explicit(exp))
        } else {
            let imp = input.parse::<ImplicitTraitAssignment>()?;
            Ok(TraitAssignment::Implicit(imp))
        }
    }
}

impl Parse for EnumToGenerate {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<Token![enum]>()?;
        let _ = input.parse::<Token![=]>()?;
        input.parse().map(Self)
    }
}

impl ToTokens for EnumToGenerate {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.0;

        tokens.extend(quote! {
            enum = #ident
        });
    }
}

impl ToTokens for ExplicitTraitAssignment {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.ident;
        let value = &self.value;

        tokens.extend(quote! {
            #ident = #value
        });
    }
}

impl ToTokens for ImplicitTraitAssignment {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let value = &self.value;

        tokens.extend(quote! {
            #value
        });
    }
}

impl ToTokens for TraitAssignment {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TraitAssignment::Implicit(i) => i.to_tokens(tokens),
            TraitAssignment::Explicit(e) => e.to_tokens(tokens),
        }
    }
}

impl ToTokens for VariantIdentifier {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            VariantIdentifier::Ident(i) => i.to_tokens(tokens),
            VariantIdentifier::Num(u) => u.to_tokens(tokens),
        }
    }
}

impl ToTokens for MapperDefinition {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(p) = &self.0 {
            tokens.extend(quote! {
                mapper = #p,
            });
        }
    }
}

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let mapper = &self.mapper;
        let fields = &self.fields;

        tokens.extend(quote! {
            #name => {
                #mapper
                #(#fields,)*
            }
        });
    }
}

impl<V: Parse> ToTokens for Variants<V> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variants = &self.0;

        tokens.extend(quote! {
            #(#variants,)*
        });
    }
}

impl ToTokens for MapperVersionsArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let enum_ident = &self.enum_to_generate;
        let revisions = &self.revisions;
        let submappers = &self.submappers;

        tokens.extend(quote! {
            #enum_ident,
            revisions {
                #revisions
            },
            submappers {
                #submappers
            }
        });
    }
}
