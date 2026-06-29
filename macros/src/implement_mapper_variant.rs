use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{ItemTrait, Token};

pub fn implement_mapper_for_struct(input: Input) -> TokenStream {
    let common = &input.common_struct;
    let enum_name = &input.enum_name;
    let variant2 = &input.variant2;
    let trait_def = &input.trait_definition;

    match variant2 {
        Variant2Version::Delegate(t) => {
            let res = quote! {
                #t!(#trait_def, #common, #enum_name);
            };

            res
        }
        Variant2Version::Trait(variant2_trait_def) => {
            quote! {}
        }
    }
}

enum Variant2Version {
    Delegate(Ident),
    Trait(ItemTrait),
}

impl Parse for Variant2Version {
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
    variant2: Variant2Version,
    common_struct: Ident,
    enum_name: Ident,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_definition = input.parse::<ItemTrait>()?;

        input.parse::<Token![,]>()?;

        let variant2 = input.parse::<Variant2Version>()?;

        input.parse::<Token![,]>()?;

        let common_struct = input.parse::<Ident>()?;

        input.parse::<Token![,]>()?;

        let enum_name = input.parse::<Ident>()?;

        Ok(Self {
            trait_definition,
            variant2,
            common_struct,
            enum_name,
        })
    }
}
