fn previous_grapheme(text: &str, offset: usize) -> usize {
    // Rounding up keeps the containing grapheme eligible for interior byte offsets.
    let offset = text.ceil_char_boundary(offset);
    GraphemeCursor::new(offset, text.len(), true)
        .prev_boundary(text, 0)
        .expect("the complete string supplies all grapheme context")
        .unwrap_or(0)
}

fn next_grapheme(text: &str, offset: usize) -> usize {
    let offset = text.floor_char_boundary(offset);
    GraphemeCursor::new(offset, text.len(), true)
        .next_boundary(text, 0)
        .expect("the complete string supplies all grapheme context")
        .unwrap_or(text.len())
}
