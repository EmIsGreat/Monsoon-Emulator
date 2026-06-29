mod implement_mapper_variant;
mod mapper_variant;
mod mapper_versions;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn mapper_variant(attribute_args: TokenStream, item: TokenStream) -> TokenStream {
    mapper_variant::mapper_variants(attribute_args.into(), item.into()).into()
}

//#[monsoon_macro::mapper_versions(
//     enum = MMC1,
//     revisions(
//         RevA => {
//             mapper = RomMapper::MMC1A,
//             name = "A"
//         },
//         RevB => {
//             mapper = RomMapper::MMC1,
//             name = "B"
//         }
//     ),
//     submappers(0, 5, 6, 7)
// )]
// #[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
// pub struct MMC1Common<V: MMC1Variant, S: MMC1Revision> {
//     ..
// }
//
#[proc_macro_attribute]
pub fn mapper_versions(attr: TokenStream, item: TokenStream) -> TokenStream {
    mapper_versions::mapper_versions(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn implement_mapper_for_struct(input: TokenStream) -> TokenStream {
    implement_mapper_variant::implement_mapper_for_struct(input.into()).into()
}
