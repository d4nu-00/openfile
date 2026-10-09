#[derive(Clone, Debug)]
pub struct ReadLimits {
    pub max_page: usize,
    pub max_game_bytes: usize,
    pub game: chess_core::GameLimits,
    pub entity_cache_bytes: usize,
}
impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            max_page: 200,
            max_game_bytes: 4 * 1024 * 1024,
            game: chess_core::GameLimits::default(),
            entity_cache_bytes: 8 * 1024 * 1024,
        }
    }
}
