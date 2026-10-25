#[cfg(test)]
mod navigation_viewport_benchmark {
    use super::*;
    use std::{hint::black_box, time::Instant};
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

    struct ViewMetadata {
        viewport_width: f32,
        viewport_height: f32,
        text_width: f32,
        scroll_y: f32,
        visible_start: usize,
        visible_end: usize,
        caret_start: usize,
        caret_end: usize,
        total_rows: usize,
    }

    fn view_metadata(
        input: &Entity<TextInputState>,
        scroll: &gpui::ScrollHandle,
        cx: &gpui::VisualTestContext,
        origin: usize,
        endpoint: usize,
    ) -> ViewMetadata {
        let viewport = scroll.bounds();
        input.read_with(cx, |input, _| {
            let layout = input.layout.borrow();
            let layout = layout.as_ref().unwrap();
            let bounds = input.text_bounds.unwrap();
            let visible_start = ((f32::from(viewport.top() - bounds.top()) / LINE_HEIGHT)
                .floor()
                .max(0.) as usize)
                .min(layout.rows.len());
            let visible_end = ((f32::from(viewport.bottom() - bounds.top()) / LINE_HEIGHT)
                .ceil()
                .max(0.) as usize)
                .min(layout.rows.len());
            assert!(
                visible_start < visible_end,
                "viewport must contain script rows"
            );
            assert_eq!(f32::from(viewport.size.width), WINDOW_WIDTH);
            assert_eq!(
                f32::from(viewport.size.height),
                WINDOW_HEIGHT - HEADER_HEIGHT - FOOTER_HEIGHT
            );
            assert_eq!(f32::from(bounds.size.width), WINDOW_WIDTH - 64.);
            ViewMetadata {
                viewport_width: f32::from(viewport.size.width),
                viewport_height: f32::from(viewport.size.height),
                text_width: f32::from(bounds.size.width),
                scroll_y: f32::from(scroll.offset().y),
                visible_start,
                visible_end,
                caret_start: layout.row_for_offset(origin),
                caret_end: layout.row_for_offset(endpoint),
                total_rows: layout.rows.len(),
            }
        })
    }

    fn position_view(
        input: &Entity<TextInputState>,
        scroll: &gpui::ScrollHandle,
        cx: &mut gpui::VisualTestContext,
        offset: f32,
        origin: usize,
        endpoint: usize,
    ) -> ViewMetadata {
        cx.update(|window, _| {
            scroll.set_offset(point(px(0.), px(-offset)));
            window.refresh();
        });
        cx.run_until_parked();
        view_metadata(input, scroll, cx, origin, endpoint)
    }

    fn center_view(
        input: &Entity<TextInputState>,
        scroll: &gpui::ScrollHandle,
        cx: &mut gpui::VisualTestContext,
        midpoint: usize,
        origin: usize,
        endpoint: usize,
    ) -> ViewMetadata {
        let row = input.read_with(cx, |input, _| {
            input
                .layout
                .borrow()
                .as_ref()
                .unwrap()
                .row_for_offset(midpoint)
        });
        let offset = (row as f32 * LINE_HEIGHT).min(f32::from(scroll.max_offset().y));
        let metadata = position_view(input, scroll, cx, offset, origin, endpoint);
        assert!(row >= metadata.visible_start && row < metadata.visible_end);
        metadata
    }

    fn print_sample(prefix: &str, metadata: &ViewMetadata, scroll_min: f32, scroll_max: f32) {
        println!(
            "{prefix},{WINDOW_WIDTH},{WINDOW_HEIGHT},{},{},{},{FONT_SIZE},{LINE_HEIGHT},{},{},{},{},{},{},{scroll_min},{scroll_max}",
            metadata.viewport_width,
            metadata.viewport_height,
            metadata.text_width,
            metadata.scroll_y,
            metadata.visible_start,
            metadata.visible_end,
            metadata.caret_start,
            metadata.caret_end,
            metadata.total_rows,
        );
    }

    #[derive(Clone, Copy)]
    enum Motion {
        Left,
        Right,
        SelectLeft,
        SelectRight,
    }

    impl Motion {
        fn name(self) -> &'static str {
            match self {
                Self::Left => "left",
                Self::Right => "right",
                Self::SelectLeft => "select_left",
                Self::SelectRight => "select_right",
            }
        }

        fn key(self) -> gpui::Keystroke {
            gpui::Keystroke::parse(match self {
                Self::Left => "left",
                Self::Right => "right",
                Self::SelectLeft => "shift-left",
                Self::SelectRight => "shift-right",
            })
            .unwrap()
        }

        fn forward(self) -> bool {
            matches!(self, Self::Right | Self::SelectRight)
        }

        fn selects(self) -> bool {
            matches!(self, Self::SelectLeft | Self::SelectRight)
        }

        fn apply(
            self,
            input: &mut TextInputState,
            window: &mut Window,
            cx: &mut Context<TextInputState>,
        ) {
            match self {
                Self::Left => input.on_move_left(&MoveLeft, window, cx),
                Self::Right => input.on_move_right(&MoveRight, window, cx),
                Self::SelectLeft => input.on_select_left(&SelectLeft, window, cx),
                Self::SelectRight => input.on_select_right(&SelectRight, window, cx),
            }
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

    fn verify_actions(input: &Entity<TextInputState>, cx: &mut gpui::VisualTestContext) {
        let text = "a🇬🇧e\u{301}👩🏽‍💻क्षb\r\n";
        let boundaries: Vec<_> = text
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect();
        input.update_in(cx, |input, _, cx| {
            input.set_text(text, cx);
            input.move_to(0, cx);
        });
        for &offset in &boundaries[1..] {
            input.update_in(cx, |input, window, cx| {
                input.on_move_right(&MoveRight, window, cx);
                assert_eq!(input.selection(), offset..offset);
            });
        }
        for &offset in boundaries[..boundaries.len() - 1].iter().rev() {
            input.update_in(cx, |input, window, cx| {
                input.on_select_left(&SelectLeft, window, cx);
                assert_eq!(input.selection(), offset..text.len());
                assert_eq!(input.cursor_offset(), offset);
                assert!(input.selection_reversed);
            });
        }
        input.update_in(cx, |input, window, cx| {
            input.on_move_right(&MoveRight, window, cx);
            assert_eq!(input.selection(), text.len()..text.len());
        });
        for &offset in boundaries[..boundaries.len() - 1].iter().rev() {
            input.update_in(cx, |input, window, cx| {
                input.on_move_left(&MoveLeft, window, cx);
                assert_eq!(input.selection(), offset..offset);
            });
        }
        for &offset in &boundaries[1..] {
            input.update_in(cx, |input, window, cx| {
                input.on_select_right(&SelectRight, window, cx);
                assert_eq!(input.selection(), 0..offset);
                assert_eq!(input.cursor_offset(), offset);
                assert!(!input.selection_reversed);
            });
        }
        input.update_in(cx, |input, window, cx| {
            input.on_move_left(&MoveLeft, window, cx);
            assert_eq!(input.selection(), 0..0);
            input.set_text("a🇬🇧b", cx);
            input.move_to(9, cx);
            input.on_backspace(&Backspace, window, cx);
            assert_eq!(input.text(), "ab");
            assert_eq!(input.selection(), 1..1);
            input.on_undo(&Undo, window, cx);
            assert_eq!(input.text(), "a🇬🇧b");
            assert_eq!(input.selection(), 9..9);
            input.move_to(1, cx);
            input.on_delete_forward(&DeleteForward, window, cx);
            assert_eq!(input.text(), "ab");
            assert_eq!(input.selection(), 1..1);
            input.on_undo(&Undo, window, cx);
            assert_eq!(input.text(), "a🇬🇧b");
            input.set_text("e\u{301}x", cx);
            input.move_to(1, cx);
            assert_eq!(input.selection(), 1..1);
            input.on_move_right(&MoveRight, window, cx);
            assert_eq!(input.selection(), 3..3);
            input.on_move_left(&MoveLeft, window, cx);
            assert_eq!(input.selection(), 0..0);
            input.set_text("éx", cx);
            input.move_to(1, cx);
            assert_eq!(input.selection(), 0..0);
            input.move_to(usize::MAX, cx);
            assert_eq!(input.selection(), 3..3);
        });
    }

    fn run_steps(
        input: &Entity<TextInputState>,
        cx: &mut gpui::VisualTestContext,
        motion: Motion,
        key: &gpui::Keystroke,
        dispatch: bool,
        steps: usize,
    ) -> f64 {
        if dispatch {
            let started = Instant::now();
            for _ in 0..steps {
                black_box(cx.update(|window, cx| window.dispatch_keystroke(key.clone(), cx)));
            }
            started.elapsed().as_secs_f64() * 1_000_000_000. / steps as f64
        } else {
            input.update_in(cx, |input, window, cx| {
                let started = Instant::now();
                for _ in 0..steps {
                    motion.apply(input, window, cx);
                }
                started.elapsed().as_secs_f64() * 1_000_000_000. / steps as f64
            })
        }
    }

    fn run_scroll_steps(
        scroll: &gpui::ScrollHandle,
        cx: &mut gpui::VisualTestContext,
        positions: &[f32],
    ) -> f64 {
        let started = Instant::now();
        for position in positions {
            cx.update(|window, _| {
                scroll.set_offset(point(px(0.), px(-*position)));
                window.refresh();
            });
        }
        started.elapsed().as_secs_f64() * 1_000_000_000. / positions.len() as f64
    }

    #[gpui::test]
    #[ignore = "manual headless TextInput action benchmark"]
    fn benchmark_text_input_navigation_viewport(cx: &mut gpui::TestAppContext) {
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
        verify_actions(&input, cx);
        let label = std::env::var("CAP_GRAPHEME_BENCH_VARIANT").unwrap_or("unlabelled".into());
        let core = std::env::var("CAP_GRAPHEME_BENCH_MATRIX").as_deref() == Ok("core");
        println!(
            "variant,kind,target_bytes,actual_bytes,graphemes,region,motion,scope,sample,steps,ns_per_step,sequence_ms,window_width,window_height,viewport_width,viewport_height,text_width,font_size,line_height,scroll_y,visible_row_start,visible_row_end,caret_row_start,caret_row_end,total_rows,scroll_offset_min,scroll_offset_max"
        );
        eprintln!(
            "viewport metadata: 560x320 window; 36px header; 44px footer; 560x240 scrolling viewport; 496px text width; 30px medium font; 45px line height; centered text; GPUI NoopTextSystem; no pixel renderer. Navigation centers the 120-step midpoint once before timing, with no per-key autoscroll. Visible row end is exclusive. Scroll cases change only ScrollHandle and request CPU redraw."
        );
        for target in [1_000, 10_000, 100_000] {
            if core && target == 1_000 {
                continue;
            }
            for kind in ["ascii", "multilingual", "emoji"] {
                let text = script(kind, target);
                let boundaries: Vec<_> = text
                    .grapheme_indices(true)
                    .map(|(offset, _)| offset)
                    .chain(std::iter::once(text.len()))
                    .collect();
                input.update_in(cx, |input, _, cx| input.set_text(text.clone(), cx));
                cx.refresh().unwrap();
                cx.run_until_parked();
                input.read_with(cx, |input, _| assert!(input.layout.borrow().is_some()));
                let steps = 120_usize.min((boundaries.len() - 1) / 2);
                for region in ["start", "middle", "end"] {
                    if core && region != "middle" {
                        continue;
                    }
                    for motion in [
                        Motion::Left,
                        Motion::Right,
                        Motion::SelectLeft,
                        Motion::SelectRight,
                    ] {
                        let pivot = match (region, motion.forward()) {
                            ("start", true) => 0,
                            ("start", false) => steps,
                            ("middle", _) => boundaries.len() / 2,
                            ("end", true) => boundaries.len() - steps - 1,
                            ("end", false) => boundaries.len() - 1,
                            _ => unreachable!(),
                        };
                        let origin = boundaries[pivot];
                        let endpoint = boundaries[if motion.forward() {
                            pivot + steps
                        } else {
                            pivot - steps
                        }];
                        let midpoint = boundaries[if motion.forward() {
                            pivot + steps / 2
                        } else {
                            pivot - steps / 2
                        }];
                        let expected = if motion.selects() {
                            origin.min(endpoint)..origin.max(endpoint)
                        } else {
                            endpoint..endpoint
                        };
                        let key = motion.key();
                        for sample in 0..=7 {
                            for dispatch in if sample % 2 == 0 {
                                [false, true]
                            } else {
                                [true, false]
                            } {
                                input.update_in(cx, |input, _, cx| input.move_to(origin, cx));
                                cx.run_until_parked();
                                let metadata =
                                    center_view(&input, &scroll, cx, midpoint, origin, endpoint);
                                let nanos = run_steps(&input, cx, motion, &key, dispatch, steps);
                                input.read_with(cx, |input, _| {
                                    assert_eq!(input.selection(), expected);
                                    assert_eq!(input.cursor_offset(), endpoint);
                                    assert_eq!(input.text(), text);
                                    assert!(input.marked_range.is_none());
                                    assert!(input.undo_stack.is_empty());
                                });
                                if sample != 0 {
                                    let prefix = format!(
                                        "{label},{kind},{target},{},{},{region},{},{},{sample},{steps},{nanos:.3},{:.6}",
                                        text.len(),
                                        boundaries.len() - 1,
                                        motion.name(),
                                        if dispatch {
                                            "key_dispatch"
                                        } else {
                                            "action_handler"
                                        },
                                        nanos * steps as f64 / 1_000_000.
                                    );
                                    print_sample(
                                        &prefix,
                                        &metadata,
                                        -metadata.scroll_y,
                                        -metadata.scroll_y,
                                    );
                                }
                            }
                        }
                    }
                }
                input.update_in(cx, |input, _, cx| input.move_to(0, cx));
                cx.run_until_parked();
                let maximum = f32::from(scroll.max_offset().y);
                for region in ["start", "middle", "end"] {
                    let origin = match region {
                        "start" => 0.,
                        "middle" => maximum / 2.,
                        "end" => maximum,
                        _ => unreachable!(),
                    };
                    let direction = if region == "end" { -1. } else { 1. };
                    let positions: Vec<_> = (0..120)
                        .map(|step| {
                            let phase = if step < 60 { step } else { 119 - step };
                            (origin + direction * LINE_HEIGHT * 2. * phase as f32 / 59.)
                                .clamp(0., maximum)
                        })
                        .collect();
                    let scroll_min = positions.iter().copied().fold(f32::INFINITY, f32::min);
                    let scroll_max = positions.iter().copied().fold(0., f32::max);
                    for sample in 0..=7 {
                        let metadata = position_view(&input, &scroll, cx, origin, 0, 0);
                        let nanos = run_scroll_steps(&scroll, cx, &positions);
                        input.read_with(cx, |input, _| {
                            assert_eq!(input.selection(), 0..0);
                            assert_eq!(input.text(), text);
                        });
                        if sample != 0 {
                            let prefix = format!(
                                "{label},{kind},{target},{},{},{region},none,scroll_redraw,{sample},120,{nanos:.3},{:.6}",
                                text.len(),
                                boundaries.len() - 1,
                                nanos * 120. / 1_000_000.,
                            );
                            print_sample(&prefix, &metadata, scroll_min, scroll_max);
                        }
                    }
                }
            }
        }
    }
}
