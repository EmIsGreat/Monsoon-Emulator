use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::{parse2, GenericParam, ItemStruct, Token, TypeParamBound};

pub fn mapper_versions(attribute_args: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_struct: ItemStruct = match parse2(item.clone()) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error(),
    };

    let struct_ident = &parsed_struct.ident;

    let variant_path = match parsed_struct.generics.params.get(0).unwrap() {
        GenericParam::Type(t) => match &t.bounds.get(0).unwrap() {
            TypeParamBound::Trait(t) => t.path.get_ident().unwrap(),
            _ => {
                panic!()
            }
        },
        _ => {
            panic!()
        }
    };

    let enum_to_generate: EnumToGenerate = match parse2(attribute_args.clone()) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error(),
    };

    let delegation = quote! {
        #variant_path!(#struct_ident, #enum_to_generate);
    };

    quote! {
        #item
        #delegation
    }
}

struct EnumToGenerate(syn::Ident);

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
