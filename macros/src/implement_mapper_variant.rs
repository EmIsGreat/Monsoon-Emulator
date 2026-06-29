use std::collections::HashMap;

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{ItemTrait, LitStr, Token, TraitItem, TraitItemConst, parse, parse_quote};

use crate::mapper_versions::{
    ImplicitTraitAssignment, MapperDefinition, MapperVersionsArgs, TraitAssignment, Variant,
    VariantIdentifier,
};

pub fn implement_mapper_for_struct(input: TokenStream) -> TokenStream {
    let input = match parse::<Input>(input.into()) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let common = &input.common_struct;
    let variant2 = &input.variant2;
    let trait_def = &input.trait_definition;
    let mapper_args = &input.args;

    match variant2 {
        Variant2Type::Delegate(t) => {
            let res = quote! {
                #t!(#trait_def, #common, #mapper_args);
            };

            res
        }
        Variant2Type::Trait(variant2_trait_def) => {
            let g = Generate::from((
                common.clone(),
                *variant2_trait_def.clone(),
                trait_def.clone(),
                mapper_args.clone(),
            ));
            quote! { #g }
        }
    }
}

impl ToTokens for Generate {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let common_name = &self.common_struct_ident;
        generate_structs_and_trait_impls(
            tokens,
            &self.primary_trait_def,
            &self.mapper_versions_args.revisions.0,
            "Rev",
        );

        generate_structs_and_trait_impls(
            tokens,
            &self.secondary_trait_def,
            &self.mapper_versions_args.submappers.0,
            "Sub",
        );

        let valid_variants = generate_enum(
            tokens,
            common_name,
            &self.mapper_versions_args,
            &self.primary_trait_def,
            &self.secondary_trait_def,
        );

        generate_enum_froms(tokens, &self.mapper_versions_args, &valid_variants)
    }
}

fn generate_enum_froms(
    tokens: &mut TokenStream,
    args: &MapperVersionsArgs,
    valid_variants: &[(Ident, MapperDefinition, Option<u8>)],
) {
    let enum_name = &args.enum_to_generate.0;

    let crate_path = get_crate_path();

    let variants = valid_variants.iter().map(|(v, m, n)| match (&m.0, n) {
        (Some(m), Some(n)) => {
            quote! {
                (#crate_path::emulation::rom::RomMapper::#m, #n) => Self::make(value, Self::#v)
            }
        }
        (Some(m), None) => {
            quote! {
                (#crate_path::emulation::rom::RomMapper::#m, _) => Self::make(value, Self::#v)
            }
        }
        (None, Some(n)) => {
            quote! {
                (_, #n) => Self::make(value, Self::#v)
            }
        }
        (None, None) => {
            quote! {
                (_, _) => Self::make(value, Self::#v)
            }
        }
    });

    tokens.extend(quote! {
        impl From<&RomFile> for #enum_name {
            fn from(value: &RomFile) -> Self {
                match (value.mapper, value.submapper_number) {
                    #(#variants, )*
                    _ => unreachable!(),
                }
            }
        }
    })
}

fn get_crate_path() -> TokenStream {
    match crate_name("monsoon-core").unwrap() {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{}", name);
            quote!(::#ident)
        }
    }
}

fn generate_enum(
    tokens: &mut TokenStream,
    common_name: &Ident,
    args: &MapperVersionsArgs,
    trait1: &ItemTrait,
    trait2: &ItemTrait,
) -> Vec<(Ident, MapperDefinition, Option<u8>)> {
    let enum_name = &args.enum_to_generate.0;
    let revisions = &args.revisions;
    let submappers = &args.submappers;
    let trait1_ident = &trait1.ident;
    let trait2_ident = &trait2.ident;

    let mut variants: Vec<TokenStream> = Vec::new();
    let mut idents = Vec::new();

    for revision in &revisions.0 {
        for submapper in &submappers.0 {
            let rev_name = &revision.name;
            let mapper = &revision.mapper;
            let sub_name = &submapper.name;

            let rev_name = match rev_name {
                VariantIdentifier::Ident(i) => i,
                VariantIdentifier::Num(n) => &format_ident!("Rev{n}"),
            };

            let ident = format_ident!("{}{}", rev_name, sub_name);

            let (sub_name, num) = match sub_name {
                VariantIdentifier::Ident(i) => (i, None),
                VariantIdentifier::Num(n) => (&format_ident!("Sub{n}"), n.base10_parse().ok()),
            };

            variants.push(quote! {
                #ident(#common_name<#rev_name, #sub_name>)
            });

            idents.push((ident, mapper.clone(), num));
        }
    }

    let crate_path = get_crate_path();

    tokens.extend(quote! {
        #[enum_delegate::implement(#crate_path::emulation::mapper::MapperLike)]
        #[derive(Debug, Clone, Eq, PartialEq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        pub enum #enum_name {
            #(#variants, )*
        }

        impl #enum_name {
            fn make<V: #trait1_ident, S: #trait2_ident>(
                value: &RomFile,
                ctor: fn(#common_name<V, S>) -> Self,
            ) -> Self {
                ctor(#common_name::from(value))
            }
        }
    });

    idents
}

fn generate_structs_and_trait_impls(
    tokens: &mut TokenStream,
    item_trait: &ItemTrait,
    variants: &Vec<Variant>,
    prefix: &str,
) {
    let trait_consts = get_fields_of_trait(item_trait);
    let trait_ident = &item_trait.ident;

    for variant in variants {
        let rev_name = match &variant.name {
            VariantIdentifier::Ident(i) => i,
            VariantIdentifier::Num(n) => &format_ident!("{prefix}{n}"),
        };

        let mut fields = variant.fields.clone();

        let substitute_name = fields.is_empty() && trait_consts.len() == 1;

        if substitute_name {
            fields.push(TraitAssignment::Implicit(ImplicitTraitAssignment {
                value: match &variant.name {
                    VariantIdentifier::Ident(i) => {
                        let lit = LitStr::new(&i.to_string(), i.span());
                        parse_quote!(#lit)
                    }
                    VariantIdentifier::Num(n) => {
                        parse_quote!(#n)
                    }
                },
            }));
        }

        let assume_field = fields.len() == 1
            && trait_consts.len() == 1
            && matches!(fields[0], TraitAssignment::Implicit(_));

        let constants: Vec<_> = if assume_field {
            let def = trait_consts.values().next().unwrap();

            let ident = &def.ident;
            let ty = &def.ty;
            let val = &fields[0];

            vec![quote! {
                const #ident: #ty = #val;
            }]
        } else {
            fields
                .iter()
                .map(|field| {
                    let field = match field {
                        TraitAssignment::Implicit(_) => {
                            panic!()
                        }
                        TraitAssignment::Explicit(e) => e,
                    };

                    let fields: Vec<_> = fields
                        .iter()
                        .filter(|f| match f {
                            TraitAssignment::Implicit(_) => {
                                tokens.extend(
                                    syn::Error::new_spanned(
                                        &variant.name,
                                        format!(
                                            "Variant `{rev_name}` is using implicit field \
                                             assignment, but trait `{trait_ident}` has more than \
                                             one field"
                                        ),
                                    )
                                    .to_compile_error(),
                                );

                                false
                            }
                            TraitAssignment::Explicit(_) => true,
                        })
                        .cloned()
                        .collect();

                    let field_idents: Vec<_> = fields
                        .iter()
                        .map(|f| match f {
                            TraitAssignment::Implicit(_) => {
                                panic!()
                            }
                            TraitAssignment::Explicit(e) => &e.ident,
                        })
                        .collect();

                    trait_consts.iter().for_each(|(i, _)| {
                        if !field_idents.contains(&i) {
                            tokens.extend(
                                syn::Error::new_spanned(
                                    &variant.name,
                                    format!(
                                        "Trait `{trait_ident}` has associated constant `{}` that \
                                         is not provided by `{rev_name}`",
                                        i
                                    ),
                                )
                                .to_compile_error(),
                            );
                        };
                    });

                    let def = match trait_consts.get(&field.ident) {
                        None => {
                            return syn::Error::new(
                                field.span(),
                                format!(
                                    "Trait `{trait_ident}` does not have associated constant `{}` \
                                     that is provided in `{rev_name}`",
                                    field.ident
                                ),
                            )
                            .to_compile_error();
                        }
                        Some(d) => d,
                    };

                    let ident = &def.ident;
                    let ty = &def.ty;
                    let val = &field.value;

                    quote! {
                        const #ident: #ty = #val;
                    }
                })
                .collect()
        };

        tokens.extend(quote! {
            #[derive(Debug, Clone, Eq, PartialEq, Hash, ::serde::Serialize, ::serde::Deserialize)]
            pub struct #rev_name;

            impl #trait_ident for #rev_name {
                #(#constants)*
            }
        });
    }
}

fn get_fields_of_trait(item_trait: &ItemTrait) -> HashMap<Ident, TraitItemConst> {
    let mut map = HashMap::new();

    for item in &item_trait.items {
        if let TraitItem::Const(c) = item {
            map.insert(c.ident.clone(), c.clone());
        }
    }

    map
}

struct Generate {
    common_struct_ident: Ident,
    primary_trait_def: ItemTrait,
    secondary_trait_def: ItemTrait,
    mapper_versions_args: MapperVersionsArgs,
}

impl From<(Ident, ItemTrait, ItemTrait, MapperVersionsArgs)> for Generate {
    fn from(
        (common_struct_ident, primary_trait_def, secondary_trait_def, mapper_versions_args): (
            Ident,
            ItemTrait,
            ItemTrait,
            MapperVersionsArgs,
        ),
    ) -> Self {
        Self {
            common_struct_ident,
            primary_trait_def,
            secondary_trait_def,
            mapper_versions_args,
        }
    }
}

enum Variant2Type {
    Delegate(Ident),
    Trait(Box<ItemTrait>),
}

impl Parse for Variant2Type {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![trait]) || input.peek2(Token![trait]) {
            Ok(Self::Trait(input.parse()?))
        } else {
            Ok(Self::Delegate(input.parse()?))
        }
    }
}

pub struct Input {
    trait_definition: ItemTrait,
    variant2: Variant2Type,
    common_struct: Ident,
    args: MapperVersionsArgs,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_definition = input.parse::<ItemTrait>()?;

        input.parse::<Token![,]>()?;

        let variant2 = input.parse::<Variant2Type>()?;

        input.parse::<Token![,]>()?;

        let common_struct = input.parse::<Ident>()?;

        input.parse::<Token![,]>()?;

        let args = input.parse::<MapperVersionsArgs>()?;

        Ok(Self {
            trait_definition,
            variant2,
            common_struct,
            args,
        })
    }
}
