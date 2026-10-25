#[cfg(test)]
mod layout_cache_diagnostic {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;

    struct Harness {
        input: Entity<TextInputState>,
        scroll: gpui::ScrollHandle,
    }

    const WINDOW_WIDTH: f32 = 560.;
    const WINDOW_HEIGHT: f32 = 320.;
    const HEADER_HEIGHT: f32 = 36.;
    const FOOTER_HEIGHT: f32 = 44.;
    const FONT_SIZE: f32 = 30.;
    const LINE_HEIGHT: f32 = 45.;

    impl gpui::Render for Harness {
        fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let spacer =
                ((f32::from(window.viewport_size().height) - HEADER_HEIGHT - FOOTER_HEIGHT) / 2.
                    - LINE_HEIGHT / 2.)
                    .max(0.);
            div()
                .size_full()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(div().h(px(HEADER_HEIGHT)).flex_shrink_0())
                .child(
                    div().flex_1().min_h_0().overflow_hidden().child(
                        div()
                            .id("navigation-benchmark-scroll")
                            .track_scroll(&self.scroll)
                            .size_full()
                            .overflow_y_scroll()
                            .child(div().h(px(spacer)).flex_shrink_0())
                            .child(
                                div().w_full().px(px(32.)).child(
                                    TextInput::bare(
                                        &Theme::dark(),
                                        "navigation-benchmark-input",
                                        &self.input,
                                    )
                                    .align(TextAlign::Center)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_size(px(FONT_SIZE))
                                    .line_height(px(LINE_HEIGHT)),
                                ),
                            )
                            .child(div().h(px(spacer)).flex_shrink_0()),
                    ),
                )
                .child(div().h(px(FOOTER_HEIGHT)).flex_shrink_0())
        }
    }

    fn script(kind: &str, target: usize) -> String {
        let sentences: &[&str] = match kind {
            "ascii" => &[
                "Welcome back. Today we will walk through the recording workflow together.",
                "First, choose the screen you want to share and check that your microphone is ready.",
                "Pause here so the audience has time to follow each step before we continue.",
                "The important detail is how the preview responds while you make an adjustment.",
                "Thanks for watching. You can return to this section whenever you need a reminder.",
            ],
            "multilingual" => &[
                "Bonjour à tous. Aujourd’hui, nous allons préparer une présentation ensemble.",
                "こんにちは。録画する画面を選んで、マイクの設定を確認してください。",
                "नमस्ते दोस्तों। आज हम इस रिकॉर्डिंग की प्रक्रिया को साथ में समझेंगे।",
                "مرحباً بكم. سنتابع الخطوات معاً ونتوقف قليلاً لشرح كل تفصيل.",
                "Gracias por acompañarnos. Revisemos cómo cambia la vista previa al editar.",
            ],
            "emoji" => &[
                "Welcome 👋🏽 to our weekly update. The team 👩🏽‍💻 is ready to share the next steps.",
                "Our colleagues in 🇬🇧 🇯🇵 🇧🇷 helped test the release. Thank you everyone!",
                "Let us pause for a cafe\u{301} ☕ and review what changed in the recording.",
                "The family 👨‍👩‍👧‍👦 will see the result together. Check the preview before continuing.",
                "We made progress 🎉, and the next topic is how to present this clearly to the audience.",
            ],
            _ => unreachable!(),
        };
        let mut seed = 0x6ab2_a603_d814_c9e5_u64;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut text = String::with_capacity(target + 256);
        while text.len() < target {
            text.push_str(sentences[random() as usize % sentences.len()]);
            text.push(if random() % 4 == 0 { '\n' } else { ' ' });
        }
        let end = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .take_while(|index| *index <= target)
            .last()
            .unwrap_or(0);
        text.truncate(end);
        text
    }

    #[derive(Debug)]
    struct LayoutSnapshot {
        wrap_width: Option<Pixels>,
        rows: usize,
        paragraphs: usize,
        font_size: Pixels,
        line_height: Pixels,
    }

    fn snapshot(layout: &InputLayout) -> LayoutSnapshot {
        LayoutSnapshot {
            wrap_width: layout.wrap_width,
            rows: layout.rows.len(),
            paragraphs: layout.lines.len(),
            font_size: layout.key.font_size,
            line_height: layout.key.line_height,
        }
    }

    fn insert_sentinel(
        input: &Entity<TextInputState>,
        cx: &mut gpui::VisualTestContext,
    ) -> LayoutSnapshot {
        input.update_in(cx, |input, _, _| {
            let mut layout = input.layout.borrow_mut();
            let layout = layout.as_mut().unwrap();
            assert_ne!(layout.paragraph_starts.last(), Some(&usize::MAX));
            let before = snapshot(layout);
            layout.paragraph_starts.push(usize::MAX);
            before
        })
    }

    fn inspect_and_remove_sentinel(
        input: &Entity<TextInputState>,
        cx: &mut gpui::VisualTestContext,
    ) -> (bool, LayoutSnapshot) {
        input.update_in(cx, |input, _, _| {
            let mut layout = input.layout.borrow_mut();
            let layout = layout.as_mut().unwrap();
            let retained = layout.paragraph_starts.last() == Some(&usize::MAX);
            if retained {
                assert_eq!(layout.paragraph_starts.pop(), Some(usize::MAX));
            }
            (retained, snapshot(layout))
        })
    }

    #[gpui::test]
    #[ignore = "manual TextInput layout cache diagnostic"]
    fn diagnose_text_input_layout_cache(cx: &mut gpui::TestAppContext) {
        cx.update(bind_keys);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| TextInputState::multi_line(window, cx));
            window.focus(&input.read(cx).focus_handle(), cx);
            Harness {
                input,
                scroll: gpui::ScrollHandle::new(),
            }
        });
        cx.simulate_resize(size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)));
        cx.run_until_parked();
        let input = view.read_with(cx, |view, _| view.input.clone());
        let scroll = view.read_with(cx, |view, _| view.scroll.clone());
        let text = script("multilingual", 10_000);
        let boundaries: Vec<_> = text
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .collect();
        let midpoint = boundaries[boundaries.len() / 2];
        input.update_in(cx, |input, _, cx| {
            input.set_text(text.clone(), cx);
            input.move_to(midpoint, cx);
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        let row = input.read_with(cx, |input, _| {
            input
                .layout
                .borrow()
                .as_ref()
                .unwrap()
                .row_for_offset(midpoint)
        });
        let scroll_offset = (row as f32 * LINE_HEIGHT).min(f32::from(scroll.max_offset().y));
        cx.update(|window, _| {
            scroll.set_offset(point(px(0.), px(-scroll_offset)));
            window.refresh();
        });
        cx.run_until_parked();

        let mut replacements = Vec::new();
        for operation in ["right", "shift-right", "scroll"] {
            let before = insert_sentinel(&input, cx);
            if operation == "scroll" {
                let offset = (scroll_offset + LINE_HEIGHT).min(f32::from(scroll.max_offset().y));
                cx.update(|window, _| {
                    scroll.set_offset(point(px(0.), px(-offset)));
                    window.refresh();
                });
            } else {
                let key = gpui::Keystroke::parse(operation).unwrap();
                assert!(cx.update(|window, cx| window.dispatch_keystroke(key, cx)));
            }
            cx.run_until_parked();
            let (retained, after) = inspect_and_remove_sentinel(&input, cx);
            eprintln!(
                "layout_cache operation={operation} retained={retained} before_wrap={:?} after_wrap={:?} before_rows={} after_rows={} before_paragraphs={} after_paragraphs={} before_font={:?} after_font={:?} before_line_height={:?} after_line_height={:?}",
                before.wrap_width,
                after.wrap_width,
                before.rows,
                after.rows,
                before.paragraphs,
                after.paragraphs,
                before.font_size,
                after.font_size,
                before.line_height,
                after.line_height
            );
            if !retained {
                replacements.push(operation);
            }
        }
        assert!(
            replacements.is_empty(),
            "warm layout replaced during: {replacements:?}"
        );
    }
}
