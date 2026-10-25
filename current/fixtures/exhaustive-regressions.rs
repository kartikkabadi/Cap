#[cfg(test)]
mod exhaustive_regressions {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;
    fn assert_grapheme_navigation_matches_boundaries(text: &str) {
        let boundaries: Vec<_> = text
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .collect();
        for offset in (0..=text.len() + 3).chain([usize::MAX]) {
            let before = boundaries.partition_point(|&boundary| boundary < offset);
            let expected_previous = before.checked_sub(1).map_or(0, |index| boundaries[index]);
            let after = boundaries.partition_point(|&boundary| boundary <= offset);
            let expected_next = boundaries.get(after).copied().unwrap_or(text.len());
            assert_eq!(
                previous_grapheme(text, offset),
                expected_previous,
                "previous at byte {offset} in {text:?}"
            );
            assert_eq!(
                next_grapheme(text, offset),
                expected_next,
                "next at byte {offset} in {text:?}"
            );
        }
    }

    #[test]
    fn grapheme_navigation_preserves_boundaries_at_every_byte_offset() {
        for text in [
            "",
            "a",
            "plain text\r\nnext line",
            "é中😀",
            "e\u{301}\u{308}x",
            "\u{301}\u{308}a",
            "a🇬🇧🇯🇵🇨b",
            "a👩🏽‍💻👨‍👩‍👧‍👦b",
            "aक्ष\u{200d}कb",
            "a\u{600}\u{600}بb",
            "a\u{1100}\u{1161}\u{11a8}b",
            "a\u{915}\u{93e}b",
            "a\u{200d}b\u{200c}c",
        ] {
            assert_grapheme_navigation_matches_boundaries(text);
        }
        for text in [
            format!("a{}b", "🇬".repeat(65)),
            format!("a{}b", "\u{301}".repeat(64)),
            format!("a{}b", "\u{600}".repeat(64)),
            format!("a{}👩b", "👩\u{200d}".repeat(32)),
        ] {
            assert_grapheme_navigation_matches_boundaries(&text);
        }
    }

    #[test]
    fn grapheme_navigation_matches_seeded_unicode_sequences() {
        let fragments = [
            "a", " ", "\r", "\n", "é", "中", "\u{301}", "\u{600}", "\u{1100}", "\u{1161}",
            "\u{11a8}", "🇬", "🇧", "👩", "🏽", "\u{200d}", "\u{200c}", "\u{915}", "\u{94d}",
            "\u{93e}",
        ];
        let mut seed = 0x6ab2_a603_d814_c9e5_u64;
        for _ in 0..256 {
            let mut text = String::new();
            for _ in 0..32 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                text.push_str(fragments[seed as usize % fragments.len()]);
            }
            assert_grapheme_navigation_matches_boundaries(&text);
        }
    }

    fn has_vertically_visible_glyph(
        line: &WrappedLine,
        origin_y: Pixels,
        line_height: Pixels,
        clip: Bounds<Pixels>,
        cx: &App,
    ) -> bool {
        let mut glyph_y = origin_y;
        let mut wraps = line.wrap_boundaries().iter().peekable();
        for (run_index, run) in line.runs().iter().enumerate() {
            let glyph_height = cx
                .text_system()
                .bounding_box(run.font_id, line.font_size())
                .size
                .height;
            for (glyph_index, _) in run.glyphs.iter().enumerate() {
                if wraps.peek().is_some_and(|boundary| {
                    boundary.run_ix == run_index && boundary.glyph_ix == glyph_index
                }) {
                    glyph_y += line_height;
                    let _ = wraps.next();
                }
                if Bounds::new(point(px(0.), glyph_y), size(px(1.), glyph_height)).intersects(
                    &Bounds::new(point(px(0.), clip.top()), size(px(1.), clip.size.height)),
                ) {
                    return true;
                }
            }
        }
        false
    }

    #[gpui::test]
    fn paragraph_culling_retains_every_clipped_glyph(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let text = "A wrapped paragraph with café and e\u{301}.\nمرحبا 👩🏽‍💻 🇬🇧 क्ष\n\n".repeat(32);
            let lines = window
                .text_system()
                .shape_text(
                    text.clone().into(),
                    px(30.),
                    &[TextRun {
                        len: text.len(),
                        font: window.text_style().font(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    Some(px(123.4)),
                    None,
                )
                .unwrap();
            let max_glyph_height = lines.iter().fold(px(0.), |height, line| {
                line.runs().iter().fold(height, |height, run| {
                    height.max(
                        cx.text_system()
                            .bounding_box(run.font_id, line.font_size())
                            .size
                            .height,
                    )
                })
            });
            let clip = Bounds::new(point(px(19.), px(31.25)), size(px(123.4), px(240.)));
            let mut omitted = 0;
            let mut visible = 0;
            assert!(max_glyph_height > px(8.));
            for line_height in [px(8.), px(16.25), px(26.4), px(45.3)] {
                let total_height = lines.iter().fold(px(0.), |height, line| {
                    height + line.size(line_height).height
                });
                for scroll in [
                    px(0.),
                    total_height / 2.,
                    (total_height - clip.size.height).max(px(0.)),
                ] {
                    let mut origin_y = clip.top() - scroll;
                    for line in &lines {
                        let accepted =
                            has_vertically_visible_glyph(line, origin_y, line_height, clip, cx);
                        let retained = paragraph_is_visible(
                            line,
                            origin_y,
                            line_height,
                            max_glyph_height,
                            clip,
                        );
                        assert!(
                            !accepted || retained,
                            "dropped a visible glyph at {origin_y:?}, height {line_height:?}"
                        );
                        omitted += usize::from(!retained);
                        visible += usize::from(accepted);
                        origin_y += line.size(line_height).height;
                    }
                }
            }
            assert!(omitted > 0);
            assert!(visible > 0);
            for line_height in [px(0.), px(-1.)] {
                assert!(paragraph_is_visible(
                    &lines[0],
                    clip.bottom() + px(100.),
                    line_height,
                    max_glyph_height,
                    clip,
                ));
            }
        });
    }

    #[gpui::test]
    fn paragraph_culling_preserves_fractional_wrap_accumulation(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let text = "a".repeat(15_000);
            let lines = window
                .text_system()
                .shape_text(
                    text.clone().into(),
                    px(30.),
                    &[TextRun {
                        len: text.len(),
                        font: window.text_style().font(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    Some(px(1.)),
                    None,
                )
                .unwrap();
            let line = &lines[0];
            assert_eq!(line.wrap_boundaries().len(), 14_999);
            let max_glyph_height = line.runs().iter().fold(px(0.), |height, run| {
                height.max(
                    cx.text_system()
                        .bounding_box(run.font_id, line.font_size())
                        .size
                        .height,
                )
            });
            let line_height = px(26.4);
            let origin_y = -line.size(line_height).height - max_glyph_height - px(1.);
            let clip = Bounds::new(point(px(0.), px(0.)), size(px(123.4), px(240.)));
            assert!(origin_y + line.size(line_height).height + max_glyph_height < clip.top());
            assert!(has_vertically_visible_glyph(
                line,
                origin_y,
                line_height,
                clip,
                cx,
            ));
            assert!(paragraph_is_visible(
                line,
                origin_y,
                line_height,
                max_glyph_height,
                clip,
            ));
        });
    }

    fn measurement_body(text: &str, window: &mut Window, cx: &mut App) -> TextBody {
        let state = cx.new(|cx| TextInputState::multi_line(window, cx));
        state.update(cx, |state, cx| state.set_text(text.to_owned(), cx));
        TextBody {
            layout: state.read(cx).shared_layout(),
            state,
            fit_content: false,
            text_size: px(30.),
            line_height: px(45.),
            weight: FontWeight::MEDIUM,
            align: TextAlign::Left,
            color: gpui::black(),
            placeholder_color: gpui::black(),
            caret_color: gpui::black(),
            selection_color: gpui::black(),
            multi_line: true,
            focused: false,
        }
    }

    fn fresh_measurement(
        body: &TextBody,
        wrap_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        let mut fresh = body.clone();
        fresh.layout = Rc::new(RefCell::new(None));
        let measured = fresh.ensure_layout(fresh.layout_key(window, cx), wrap_width, window, cx);
        size(wrap_width.unwrap_or(measured.width), measured.height)
    }

    #[gpui::test]
    fn measurement_matches_rows_without_replacing_the_active_layout(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            for text in [
                "",
                "\n\n",
                "A paragraph with repeated words and wrapping.\nSecond paragraph.\n",
                "مرحبا café e\u{301} हिन्दी 👩🏽‍💻 🇬🇧\n中文标点。\n",
            ] {
                let mut body = measurement_body(text, window, cx);
                for line_height in [px(16.25), px(45.)] {
                    body.line_height = line_height;
                    for wrap_width in [None, Some(px(0.)), Some(px(1.)), Some(px(70.5))] {
                        assert_eq!(
                            body.measure(&body.layout_key(window, cx), wrap_width, window),
                            fresh_measurement(&body, wrap_width, window, cx),
                        );
                        assert!(body.layout.borrow().is_none());
                    }
                }
                body.ensure_layout(body.layout_key(window, cx), Some(px(496.)), window, cx);
                body.layout.borrow_mut().as_mut().unwrap().max_glyph_height = px(-123.);
                for wrap_width in [None, None, Some(px(1368.)), Some(px(496.)), Some(px(496.))] {
                    assert_eq!(
                        body.measure(&body.layout_key(window, cx), wrap_width, window),
                        fresh_measurement(&body, wrap_width, window, cx),
                    );
                    body.ensure_layout(body.layout_key(window, cx), Some(px(496.)), window, cx);
                    let cached = body.layout.borrow();
                    let cached = cached.as_ref().unwrap();
                    assert_eq!(cached.wrap_width, Some(px(496.)));
                    assert_eq!(cached.max_glyph_height, px(-123.));
                }
                for width in 1..=20 {
                    let wrap_width = Some(px(width as f32 * 40.));
                    assert_eq!(
                        body.measure(&body.layout_key(window, cx), wrap_width, window),
                        fresh_measurement(&body, wrap_width, window, cx),
                    );
                    assert!(body.layout.borrow().as_ref().unwrap().measurements.len() <= 4);
                }
                assert!(
                    body.layout
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .measurements
                        .iter()
                        .all(|(width, _)| *width != px(40.))
                );
                assert_eq!(
                    body.measure(&body.layout_key(window, cx), Some(px(40.)), window),
                    fresh_measurement(&body, Some(px(40.)), window, cx),
                );
                let cached = body.layout.borrow();
                let cached = cached.as_ref().unwrap();
                assert_eq!(cached.measurements.len(), 4);
                assert_eq!(cached.measurements.last().unwrap().0, px(40.));
                assert_eq!(cached.max_glyph_height, px(-123.));
            }
        });
    }

    fn assert_measurement_layout_rebuilt(
        body: &TextBody,
        wrap_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        body.layout.borrow_mut().as_mut().unwrap().max_glyph_height = px(-123.);
        let expected = fresh_measurement(body, Some(px(1368.)), window, cx);
        assert_eq!(
            body.measure(&body.layout_key(window, cx), Some(px(1368.)), window),
            expected
        );
        body.ensure_layout(body.layout_key(window, cx), wrap_width, window, cx);
        let cached = body.layout.borrow();
        let cached = cached.as_ref().unwrap();
        assert_eq!(cached.key, body.layout_key(window, cx));
        assert_eq!(cached.wrap_width, wrap_width);
        assert_ne!(cached.max_glyph_height, px(-123.));
        assert!(cached.measurements.is_empty());
    }

    #[gpui::test]
    fn measurement_cache_tracks_resize_text_and_style(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.), px(0.)),
            size(px(800.), px(600.)),
            |window, cx| {
                let mut body =
                    measurement_body("A paragraph with several words to wrap.", window, cx);
                body.ensure_layout(body.layout_key(window, cx), Some(px(496.)), window, cx);
                body.measure(&body.layout_key(window, cx), Some(px(1368.)), window);
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);

                body.state.update(cx, |state, _| {
                    state.content =
                        "Changed text with additional words and a new paragraph.\nNext.".into();
                });
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                body.text_size = px(24.);
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                body.line_height = px(36.5);
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                body.weight = FontWeight::BOLD;
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                body.color = gpui::white();
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                body.state
                    .update(cx, |state, _| state.marked_range = Some(0..7));
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                body.state.update(cx, |state, _| state.marked_range = None);
                assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);

                for style in [
                    gpui::TextStyleRefinement {
                        font_family: Some("measurement-test-font".into()),
                        ..Default::default()
                    },
                    gpui::TextStyleRefinement {
                        font_style: Some(gpui::FontStyle::Italic),
                        ..Default::default()
                    },
                ] {
                    window.with_text_style(Some(style), |window| {
                        assert_measurement_layout_rebuilt(&body, Some(px(250.)), window, cx);
                    });
                }
                div()
            },
        );
    }

    #[gpui::test]
    fn deferred_measurement_retains_the_inherited_font(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        for fit_content in [false, true] {
            let mut body = cx.update(|window, cx| {
                measurement_body(
                    "A paragraph with several words to wrap. "
                        .repeat(8)
                        .as_str(),
                    window,
                    cx,
                )
            });
            body.fit_content = fit_content;
            let draw = |cx: &mut gpui::VisualTestContext| {
                cx.draw(
                    point(px(0.), px(0.)),
                    size(
                        gpui::AvailableSpace::Definite(px(496.)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    |_, _| {
                        div()
                            .w(px(496.))
                            .flex()
                            .flex_col()
                            .items_start()
                            .font_family("measurement-parent-font")
                            .child(body.clone())
                    },
                );
            };
            draw(cx);
            {
                let mut cached = body.layout.borrow_mut();
                let cached = cached.as_mut().unwrap();
                assert_eq!(cached.key.font.family.as_ref(), "measurement-parent-font");
                assert_eq!(cached.wrap_width, (!fit_content).then_some(px(496.)));
                cached.max_glyph_height = px(-123.);
                if !fit_content {
                    cached.size.height = px(123.);
                }
            }
            draw(cx);
            assert_eq!(
                body.layout.borrow().as_ref().unwrap().max_glyph_height,
                px(-123.)
            );
            if !fit_content {
                assert_eq!(
                    cx.update(|_, cx| body.state.read(cx).text_bounds.unwrap().size.height),
                    px(123.),
                );
            }
        }
    }

}
