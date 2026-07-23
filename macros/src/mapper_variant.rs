use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{ItemTrait, TraitItem, parse2};

pub fn mapper_variants(attribute_args: &TokenStream, item: TokenStream) -> TokenStream {
    if !attribute_args.is_empty() {
        let span = attribute_args.span();
        return quote_spanned!(span => compile_error!("this macro doesn't accept any arguments"););
    }

    let parsed_trait: ItemTrait = match parse2(item) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error(),
    };
    let cleaned_trait = with_helper_attributes_removed(&parsed_trait);

    let random_id = get_rand_id();
    let macro_name = format_ident!("mapper_variants_{}_{}", &parsed_trait.ident, random_id);

    let trait_name = &parsed_trait.ident;

    quote! {
            #[doc(hidden)]
            #[macro_export]
            macro_rules! #macro_name {
                ($($tokens:tt)*) => {
                    ::monsoon_macro::implement_mapper_for_struct! {
                        #cleaned_trait,
                        $($tokens)*
                    }
                };
            }

            #[doc(hidden)]
            pub use #macro_name as #trait_name;

            #cleaned_trait
    }
}

fn with_helper_attributes_removed(original: &ItemTrait) -> ItemTrait {
    let mut cleaned = original.clone();

    cleaned.items.iter_mut().for_each(|item| {
        if let TraitItem::Type(t) = item {
            let attributes = &mut t.attrs;

            let mut i = 0;
            while i < attributes.len() {
                if attributes[i]
                    .path()
                    .is_ident(&format_ident!("mapper_variants"))
                {
                    let _ = attributes.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    });

    cleaned
}

fn get_rand_id() -> String {
    use rand::RngExt;
    use rand::distr::Alphanumeric;

    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(42)
        .map(char::from)
        .collect()
}
