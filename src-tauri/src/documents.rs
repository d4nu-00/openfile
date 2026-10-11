pub struct Document {
    pub game: chess_core::Game,
    pub revision: u64,
    pub dirty: bool,
    pub incomplete_annotations: bool,
}
