pub mod class_path_profiling;
pub mod search_mapper;
pub mod three_sum;

fn main() {
    search_mapper::print_stats(|f| Some(f.mapper), true);
}
