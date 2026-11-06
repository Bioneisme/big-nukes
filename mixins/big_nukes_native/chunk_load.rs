fn big_nukes_chunk_load(game: &mut crate::game::Game<'_>, dimension: &str, chunk: [i32; 2]) {
    crate::game::big_nukes_glue::chunk_load(game, dimension, chunk);
}
