use crate::search_mapper::get_transformer;

pub mod search_mapper;

fn main() {
    search_mapper::print_with_predicate(
        |r| r.mapper.to_string().len() == 90,
        &[get_transformer(|r| r.mapper.to_string())],
        true,
    );
}
