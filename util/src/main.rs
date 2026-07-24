pub mod search_mapper;

fn main() {
    search_mapper::print_stats(|f| Some(f.mapper), true);
}
