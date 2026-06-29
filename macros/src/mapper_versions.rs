use std::marker::PhantomData;

use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Token;
use syn::{
    braced, parse2, Expr, GenericParam, Generics, ItemStruct, LitInt, Path, Token,
    TypeParamBound, WherePredicate,
};

pub fn mapper_versions(attribute_args: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_struct: ItemStruct = match parse2(item.clone()) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error(),
    };

    let struct_ident = parsed_struct.ident.clone();

    let (variant1_path, variant2_path) = match parse_generics(parsed_struct) {
        Ok(v) => v,
        Err(err) => return err.to_compile_error(),
    };

    let parsed_input: Input = match parse2(attribute_args.clone()) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error(),
    };

    let enum_ident = &parsed_input.enum_to_generate;

    let delegation = quote! {
        #variant1_path!(#variant2_path, #struct_ident, #enum_ident);
    };

    quote! {
        #item
        #delegation
    }
}

const GENERICS_ERROR_MESSAGE: &'static str = "`#[mapper_versions]` could not determine the mapper \
                                              variant traits.
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

fn parse_generics(parsed_struct: ItemStruct) -> Result<(Ident, Ident), syn::Error> {
    if parsed_struct.generics.params.len() < 2 {
        return Err(syn::Error::new_spanned(
            &parsed_struct.ident,
            GENERICS_ERROR_MESSAGE,
        ));
    }

    let valid_variant_bounds = get_valid_variant_bounds(parsed_struct.generics)?;

    if valid_variant_bounds.len() < 2 {
        return Err(syn::Error::new_spanned(
            &parsed_struct.ident,
            GENERICS_ERROR_MESSAGE,
        ));
    };

    Ok((
        valid_variant_bounds[0].clone(),
        valid_variant_bounds[1].clone(),
    ))
}

fn get_valid_variant_bounds(generics: Generics) -> Result<Vec<Ident>, syn::Error> {
    let mut res = Vec::new();

    for param in &generics.params {
        match param {
            GenericParam::Type(t) => {
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
                        res.push(where_bounds[0].clone())
                    }
                } else {
                    res.push(bounds[0].clone())
                }
            }
            _ => {}
        }
    }

    Ok(res)
}

mod kw {
    syn::custom_keyword!(revisions);
    syn::custom_keyword!(submappers);
}

#[derive(Debug)]
struct Input {
    enum_to_generate: EnumToGenerate,
    revisions: Variants<kw::revisions>,
    submappers: Variants<kw::submappers>,
}

#[derive(Debug)]
struct EnumToGenerate(Ident);

#[derive(Debug)]
struct Variants<V: Parse>(Vec<Variant>, PhantomData<V>);

#[derive(Debug)]
struct Variant {
    name: VariantIdentifier,
    mapper: Option<Path>,
    fields: Vec<TraitAssignment>,
}

#[derive(Debug)]
struct TraitAssignment {
    ident: Ident,
    value: Expr,
}

#[derive(Debug)]
enum VariantIdentifier {
    Ident(Ident),
    Num(LitInt),
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

impl Parse for Input {
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

        Ok(Self(variants, PhantomData::default()))
    }
}

impl Parse for Variant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<VariantIdentifier>()?;
        let mut mapper = None;
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
                if assignment.ident == "mapper" {
                    let val = &assignment.value;
                    mapper = Some(parse2(quote::quote!(#val))?);
                } else {
                    fields.push(assignment);
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

impl Parse for TraitAssignment {
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
            #ident
        })
    }
}
