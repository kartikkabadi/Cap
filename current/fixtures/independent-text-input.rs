#[cfg(test)]
mod independent_review {
    use super::*;
    use serde_json::json;

    fn body(text: &str, multiline: bool, window: &mut Window, cx: &mut App) -> TextBody {
        let state = cx.new(|cx| {
            if multiline {
                TextInputState::multi_line(window, cx)
            } else {
                TextInputState::single_line(window, cx)
            }
        });
        state.update(cx, |state, cx| state.set_text(text.to_owned(), cx));
        TextBody {
            layout: state.read(cx).shared_layout(), state,
            fit_content: false, text_size: px(17.), line_height: px(26.4),
            weight: FontWeight::NORMAL, align: TextAlign::Left,
            color: gpui::black(), placeholder_color: gpui::black(),
            caret_color: gpui::black(), selection_color: gpui::black(),
            multi_line: multiline, focused: true,
        }
    }

    fn draw(body: &TextBody, width: f32, cx: &mut gpui::VisualTestContext) {
        cx.draw(point(px(3.), px(7.)),
            size(gpui::AvailableSpace::Definite(px(width)), gpui::AvailableSpace::MaxContent),
            |_, _| body.clone());
    }

    fn snapshot(body: &TextBody, cx: &mut gpui::VisualTestContext) -> serde_json::Value {
        cx.update(|_, cx| {
            let state = body.state.read(cx);
            let layout = body.layout.borrow();
            let layout = layout.as_ref().unwrap();
            let rows: Vec<_> = layout.rows.iter().map(|row|
                json!([row.range.start, row.range.end, row.paragraph, row.local_start, f32::from(row.width)])
            ).collect();
            let positions: Vec<_> = state.content.char_indices().map(|(index, _)| {
                let row = layout.row_for_offset(index);
                let x = layout.x_for_offset(index, body.line_height);
                let hit = layout.offset_for_point(point(x, body.line_height * row as f32), body.line_height);
                json!([index, row, f32::from(x), hit])
            }).collect();
            let bounds = state.text_bounds.unwrap();
            json!({"text": state.text(), "selection": [state.selected_range.start, state.selected_range.end],
                "reversed": state.selection_reversed, "marked": state.marked_range,
                "bounds": [f32::from(bounds.origin.x), f32::from(bounds.origin.y), f32::from(bounds.size.width), f32::from(bounds.size.height)],
                "rows": rows, "positions": positions, "paragraph_starts": layout.paragraph_starts,
                "paragraph_rows": layout.paragraph_rows, "scroll_x": f32::from(state.scroll_x)})
        })
    }

    #[gpui::test]
    fn independent_geometry_and_hit_test_oracle(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        let mut output = Vec::new();
        for text in ["", "\n\n", "hello world", "first paragraph has many words that wrap.\nSecond paragraph.\n", "café e\u{301} 中文 हिन्दी مرحبا 👩🏽‍💻 🇬🇧\nnext" ] {
            for multiline in [false, true] {
                for fit in [false, true] {
                    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                        let mut body = cx.update(|w, cx| body(text, multiline, w, cx));
                        body.fit_content = fit;
                        body.align = align;
                        for width in [80.5, 496., 113.25, 80.5] {
                            draw(&body, width, cx);
                            let cold = snapshot(&body, cx);
                            draw(&body, width, cx);
                            assert_eq!(snapshot(&body, cx), cold, "cold and warm geometry differs");
                            output.push(cold);
                            body.state.update_in(cx, |s, _, cx| s.select_all_text(cx));
                            draw(&body, width, cx);
                            output.push(snapshot(&body, cx));
                            body.state.update_in(cx, |s, _, cx| s.move_to_end(cx));
                        }
                    }
                }
            }
        }
        if let Ok(path) = std::env::var("CAP_REVIEW_SNAPSHOT") {
            std::fs::write(path, serde_json::to_vec(&output).unwrap()).unwrap();
        }
        assert_eq!(output.len(), 480);
    }

    #[gpui::test]
    fn independent_composition_editing_and_undo(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        let body = cx.update(|w, cx| body("prefix suffix", true, w, cx));
        draw(&body, 100., cx);
        body.state.update_in(cx, |s, w, cx| {
            s.move_to(7, cx);
            s.replace_and_mark_text_in_range(None, "に", Some(1..1), w, cx);
            assert_eq!(s.text(), "prefix にsuffix");
            assert!(s.marked_range.is_some());
        });
        draw(&body, 100., cx);
        body.state.update_in(cx, |s, w, cx| {
            s.replace_and_mark_text_in_range(None, "日本", Some(2..2), w, cx);
            assert_eq!(s.text(), "prefix 日本suffix");
        });
        draw(&body, 100., cx);
        body.state.update_in(cx, |s, w, cx| {
            s.replace_text_in_range(None, "日本語", w, cx);
            assert_eq!(s.text(), "prefix 日本語suffix");
            s.unmark_text(w, cx);
            assert!(s.marked_range.is_none());
        });
        draw(&body, 100., cx);
        cx.update(|w, cx| {
            body.state.update(cx, |s, cx| {
                s.on_undo(&Undo, w, cx);
                assert_eq!(s.text(), "prefix 日本suffix");
                s.on_undo(&Undo, w, cx);
                assert_eq!(s.text(), "prefix suffix");
                s.on_redo(&Redo, w, cx);
                assert_eq!(s.text(), "prefix 日本suffix");
                s.on_redo(&Redo, w, cx);
                assert_eq!(s.text(), "prefix 日本語suffix");
                s.set_text("a👩🏽‍💻e\u{301}b", cx);
                s.move_to_end(cx);
                s.on_backspace(&Backspace, w, cx);
                s.on_backspace(&Backspace, w, cx);
                assert_eq!(s.text(), "a👩🏽‍💻");
                s.move_to(1, cx);
                s.on_delete_forward(&DeleteForward, w, cx);
                assert_eq!(s.text(), "a");
            });
        });
        draw(&body, 100., cx);
    }
}
