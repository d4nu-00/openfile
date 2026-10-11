// Test-only coordinate encoder; does not use production PieceState or tables.
// Permutation derived from MIT cbh2pgn, Copyright (c) 2022 Dominik Klein.
const INVERSE: [u8; 256] = [
    170, 73, 57, 216, 93, 194, 177, 178, 71, 118, 181, 165, 184, 203, 83, 127, 107, 141, 121, 190,
    235, 33, 153, 210, 87, 77, 180, 191, 98, 189, 36, 150, 167, 72, 40, 110, 47, 90, 24, 78, 248,
    67, 215, 99, 156, 230, 46, 198, 38, 136, 48, 97, 111, 20, 169, 104, 238, 251, 119, 226, 166, 5,
    139, 161, 152, 50, 82, 2, 151, 225, 65, 195, 124, 228, 6, 183, 85, 217, 44, 174, 55, 246, 63,
    8, 147, 115, 94, 120, 53, 242, 109, 113, 162, 243, 22, 88, 61, 250, 233, 186, 212, 221, 74,
    196, 14, 254, 95, 117, 7, 137, 52, 45, 193, 142, 245, 100, 23, 112, 164, 123, 218, 224, 133,
    197, 11, 144, 249, 132, 255, 21, 54, 9, 158, 125, 222, 187, 223, 188, 58, 18, 51, 19, 25, 229,
    148, 80, 17, 234, 49, 1, 92, 149, 202, 211, 29, 126, 239, 68, 128, 160, 31, 131, 0, 75, 103,
    32, 91, 42, 146, 182, 96, 26, 66, 15, 13, 176, 209, 35, 240, 122, 84, 79, 244, 168, 114, 231,
    64, 56, 89, 135, 232, 108, 134, 4, 241, 140, 206, 106, 219, 129, 130, 154, 27, 157, 10, 43,
    143, 205, 237, 16, 116, 105, 214, 81, 185, 69, 59, 86, 145, 253, 171, 102, 62, 70, 179, 252,
    200, 155, 192, 227, 163, 172, 201, 236, 39, 41, 159, 37, 199, 204, 101, 76, 213, 30, 207, 3,
    138, 175, 247, 173, 60, 208, 34, 28, 220, 12,
];
pub fn coordinate_word(uci: &str) -> u16 {
    let b = uci.as_bytes();
    let square = |f: u8, r: u8| ((f - b'a') as u16) * 8 + (r - b'1') as u16;
    let promo = match b.get(4) {
        Some(b'r') => 1,
        Some(b'b') => 2,
        Some(b'n') => 3,
        _ => 0,
    };
    square(b[0], b[1]) | (square(b[2], b[3]) << 6) | (promo << 12)
}
pub fn two_byte(body: &mut Vec<u8>, count: &mut u8, uci: &str) {
    let word = coordinate_word(uci).to_be_bytes();
    body.push(0x29u8.wrapping_add(*count));
    body.push(INVERSE[word[0] as usize].wrapping_add(*count));
    body.push(INVERSE[word[1] as usize].wrapping_add(*count));
    *count = count.wrapping_add(1);
}
pub fn primary(body: &mut Vec<u8>, count: &mut u8, token: u8) {
    body.push(token.wrapping_add(*count));
    *count = count.wrapping_add(1);
}
pub fn finish(mut body: Vec<u8>, count: u8) -> Vec<u8> {
    body.push(0x0cu8.wrapping_add(count));
    let mut record = ((body.len() + 4) as u32).to_be_bytes().to_vec();
    record.extend(body);
    record
}
pub fn line(moves: &[&str]) -> Vec<u8> {
    let mut body = vec![];
    let mut count = 0;
    for uci in moves {
        two_byte(&mut body, &mut count, uci);
    }
    finish(body, count)
}
