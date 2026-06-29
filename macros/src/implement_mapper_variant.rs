use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, ItemTrait, Token};

pub fn implement_mapper_for_struct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    let common = &input.common_struct;
    let enum_name = &input.enum_name;

    quote! {
        #[derive(Debug)]
        pub enum #enum_name {
            Dummy(#common<RevA, 0>),
        }
    }
    .into()
}

struct Input {
    trait_definition: ItemTrait,
    common_struct: Ident,
    enum_name: Ident,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_definition = input.parse::<ItemTrait>()?;

        input.parse::<Token![,]>()?;

        let common_struct = input.parse::<Ident>()?;

        input.parse::<Token![,]>()?;

        let enum_name = input.parse::<Ident>()?;

        Ok(Self {
            trait_definition,
            common_struct,
            enum_name,
        })
    }
}
