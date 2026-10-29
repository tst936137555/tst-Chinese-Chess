use super::*;
impl App {
    pub(in crate::ui) fn review_screen(&mut self) {
        let Some(r) = &self.review else {
            return;
        };
        let ply = r.ply;
        let n = r.game.history.len();
        let busy = r.analyzing;
        let board = r.game.at_ply(ply).unwrap().clone();
        let last = ply.checked_sub(1).map(|i| r.game.history[i].mv);
        self.board(&board, last, false);
        let (x, w) = self.content();
        let split = self.landscape();
        let card_y = if split { 100. } else { 544. };
        let card_h = if split && self.height < 560. {
            100.
        } else if split {
            124.
        } else {
            84.
        };
        self.card(Rect::new(x, card_y, w, card_h));
        let title = if ply == 0 {
            "初始局面".into()
        } else {
            format!(
                "{ply}. {}",
                self.review.as_ref().unwrap().game.history[ply - 1].notation
            )
        };
        self.fit_text(&title, x + 16., card_y + 31., w - 118., 22., INK);
        if self.engine_failed {
            if self.button(
                "重试分析",
                Rect::new(x + w - 99., card_y + 10., 86., 37.),
                false,
                true,
            ) {
                self.engine_failed = false;
                self.notice.clear();
                self.review.as_mut().unwrap().analyzing = true;
            }
        } else if self
            .review
            .as_ref()
            .is_some_and(|r| r.details[ply].as_ref().is_some_and(|d| !d.line.is_empty()))
        {
            if self.button(
                "参考变化",
                Rect::new(x + w - 99., card_y + 10., 86., 37.),
                false,
                true,
            ) {
                let r = self.review.as_ref().unwrap();
                let details = r.details[ply].as_ref().unwrap();
                let mut position = board.clone();
                let mut words = Vec::new();
                for (index, word) in details.line.iter().take(16).enumerate() {
                    let Ok(m) = word.parse() else { break };
                    let Ok(notation) = crate::notation::chinese(&position, m) else {
                        break;
                    };
                    if position.play(m).is_err() {
                        break;
                    }
                    words.push(format!(
                        "{}. {}{}",
                        index + 1,
                        position.turn.other().label(),
                        notation
                    ));
                }
                let text = format!(
                    "{}\n\n{}",
                    r.scores[ply].map_or_else(|| "待分析".into(), Score::label),
                    words.join("\n")
                );
                self.open_modal(Modal::Variation(text));
            }
        } else if self.review.as_ref().is_some_and(|r| r.bests[ply].is_some()) {
            // Older caches retain useful scores but have no principal variation.
            // Refresh only this position on demand, preserving its current score.
            if self.button(
                "补充变化",
                Rect::new(x + w - 99., card_y + 10., 86., 37.),
                false,
                self.pending.is_none(),
            ) {
                self.search(Job::Review(ply));
            }
        } else {
            self.fit_text(
                &format!("{ply} / {n} 步"),
                x + w - 95.,
                card_y + 29.,
                80.,
                15.,
                MUTED,
            );
        }
        let r = self.review.as_ref().unwrap();
        let mut detail = "点击走势可跳转到任意一步".to_owned();
        if ply > 0 {
            let e = &r.game.history[ply - 1];
            let assessment = r.assessment(ply);
            let quality = assessment.map(|a| a.label());
            detail = quality.map_or_else(
                || "着法评价将在分析后显示".into(),
                |q| format!("按当前分析：{q}"),
            );
            if let Some(m) = r.bests[ply - 1] {
                if m == e.mv && assessment != Some(crate::engine::MoveAssessment::AllowsMate) {
                    detail.push_str(" · 最佳走法");
                } else if m != e.mv {
                    detail.push_str(&format!(
                        " · 建议 {}",
                        crate::notation::chinese(r.game.at_ply(ply - 1).unwrap(), m)
                            .unwrap_or_else(|_| m.to_string())
                    ));
                }
            }
        }
        self.fit_text(&detail, x + 16., card_y + 64., w - 32., 16., JADE);
        if split && self.height >= 560. {
            self.fit_text(
                "随时翻页，后台分析不影响当前局面",
                x + 16.,
                card_y + 99.,
                w - 32.,
                15.,
                MUTED,
            );
        }
        let r = self.review.as_ref().unwrap();
        let done = (0..r.scores.len()).filter(|&i| r.complete(i)).count();
        let score = r.scores[ply].map_or_else(|| "局势待分析".into(), Score::label);
        let progress_y = if split && self.height < 560. {
            220.
        } else if split {
            263.
        } else {
            663.
        };
        self.panel(Rect::new(x, progress_y, w, 4.), BORDER);
        if done > 0 {
            self.panel(
                Rect::new(x, progress_y, w * done as f32 / (n + 1) as f32, 4.),
                JADE,
            );
        }
        let analysis_state = if self.engine_failed {
            "分析失败"
        } else if done == n + 1 {
            "分析完成"
        } else if busy {
            "正在分析"
        } else {
            "已暂停"
        };
        self.fit_text(
            &format!("{analysis_state} · {done}/{}", n + 1),
            x + 2.,
            progress_y - 9.,
            w * 0.54,
            14.,
            MUTED,
        );
        let status = if r.scores[ply].is_some() {
            format!("{score} · 深度 {}", r.depths[ply])
        } else {
            score
        };
        self.fit_text(
            &status,
            x + w * 0.54,
            progress_y - 9.,
            w * 0.46 - 2.,
            14.,
            MUTED,
        );
        let chart_y = if split && self.height < 560. {
            240.
        } else if split {
            284.
        } else {
            683.
        };
        let chart_bottom = if split {
            self.height - 170.
        } else {
            self.height - 108.
        };
        self.draw_review_chart(Rect::new(x, chart_y, w, chart_bottom - chart_y));
        let mode_y = if split { self.height - 156. } else { 96. };
        if self.button(
            if done == n + 1 {
                "分析完成"
            } else if busy {
                "暂停分析"
            } else {
                "继续分析"
            },
            Rect::new(x, mode_y, w, 40.),
            false,
            done < n + 1,
        ) {
            if busy {
                self.save_review_cache();
                self.engine.cancel();
                self.pending = None;
                self.review.as_mut().unwrap().analyzing = false;
            } else {
                self.engine_failed = false;
                self.review.as_mut().unwrap().analyzing = true;
            }
        }
        let controls_y = self.height - 82.;
        let bw = (w - 24.) / 4.;
        let mut target = ply;
        for (i, label, dest, enabled) in [
            (0, "首步", 0, ply > 0),
            (1, "上一步", ply.saturating_sub(1), ply > 0),
            (2, "下一步", (ply + 1).min(n), ply < n),
            (3, "末步", n, ply < n),
        ] {
            if self.button(
                label,
                Rect::new(x + i as f32 * (bw + 8.), controls_y, bw, 54.),
                false,
                enabled,
            ) {
                target = dest;
            }
        }
        if self.modal.is_none() {
            if is_key_pressed(KeyCode::Left) {
                target = ply.saturating_sub(1);
            }
            if is_key_pressed(KeyCode::Right) {
                target = (ply + 1).min(n);
            }
            if is_key_pressed(KeyCode::Home) {
                target = 0;
            }
            if is_key_pressed(KeyCode::End) {
                target = n;
            }
        }
        // The chart may already have changed the cursor this frame.
        if target != ply {
            self.review.as_mut().unwrap().ply = target;
        }
    }
    pub(in crate::ui) fn draw_review_chart(&mut self, chart: Rect) {
        self.card(chart);
        self.text("局势走势", chart.x + 12., chart.y + 23., 15., INK);
        for (offset, label, color) in [(108., "红优", RED_INK), (54., "黑优", BLACK)] {
            let x = chart.x + chart.w - offset;
            draw_line(x, chart.y + 19., x + 12., chart.y + 19., 2.5, color);
            self.text(label, x + 17., chart.y + 23., 13., color);
        }
        let plot = Rect::new(
            chart.x + 14.,
            chart.y + 34.,
            chart.w - 28.,
            (chart.h - 46.).max(20.),
        );
        let r = self.review.as_mut().unwrap();
        let columns = plot.w.ceil().max(1.) as usize;
        if r.chart_cache.version != r.score_version || r.chart_cache.columns != columns {
            let mistakes = r.mistake_plies();
            r.chart_cache = crate::review::ChartCache {
                version: r.score_version,
                columns,
                points: crate::review::marked_chart_samples(&r.scores, columns, &mistakes),
                mistakes,
            };
        }
        let r = self.review.as_ref().unwrap();
        let n = r.game.history.len();
        let middle = plot.y + plot.h / 2.;
        for fraction in [0.25, 0.5, 0.75] {
            let y = plot.y + plot.h * fraction;
            draw_line(
                plot.x,
                y,
                plot.x + plot.w,
                y,
                1.,
                if fraction == 0.5 { BORDER } else { PAPER },
            );
        }
        let point = |i: usize, cp: i32| {
            vec2(
                plot.x + i as f32 / n.max(1) as f32 * plot.w,
                middle - (cp as f32 / 600.).tanh() * plot.h * 0.44,
            )
        };
        let advantage_color = |y: f32| {
            if y < middle {
                RED_INK
            } else if y > middle {
                BLACK
            } else {
                MUTED
            }
        };
        let segment = |a: Vec2, b: Vec2| {
            let color = advantage_color((a.y + b.y) / 2.);
            let fill = Color::new(color.r, color.g, color.b, 0.08);
            draw_triangle(vec2(a.x, middle), a, b, fill);
            draw_triangle(vec2(a.x, middle), b, vec2(b.x, middle), fill);
            draw_line(a.x, a.y, b.x, b.y, 2.5, color);
        };
        for pair in r.chart_cache.points.windows(2) {
            if let (Some(a), Some(b)) = (pair[0].1, pair[1].1) {
                let a = point(pair[0].0, a.value());
                let b = point(pair[1].0, b.value());
                if (a.y < middle && b.y > middle) || (a.y > middle && b.y < middle) {
                    let crossing = vec2(a.x + (b.x - a.x) * (middle - a.y) / (b.y - a.y), middle);
                    segment(a, crossing);
                    segment(crossing, b);
                } else {
                    segment(a, b);
                }
            }
        }
        let cursor = plot.x + r.ply as f32 / n.max(1) as f32 * plot.w;
        draw_line(cursor, plot.y, cursor, plot.y + plot.h, 1.5, GOLD);
        for &ply in &r.chart_cache.mistakes {
            if let Some(score) = r.scores[ply] {
                let p = point(ply, score.value());
                draw_circle(p.x, p.y, 4.5, SURFACE);
                draw_circle(p.x, p.y, 3., RED_INK);
            }
        }
        if let Some(score) = r.scores[r.ply] {
            let p = point(r.ply, score.value());
            draw_circle_lines(p.x, p.y, 6., 1.5, GOLD);
            if !r.chart_cache.mistakes.contains(&r.ply) {
                draw_circle(p.x, p.y, 4.5, SURFACE);
                draw_circle(p.x, p.y, 3., advantage_color(p.y));
            }
        }
        if r.scores.iter().all(Option::is_none) {
            self.center(
                "分析后显示局势变化",
                plot.x + plot.w / 2.,
                middle,
                14.,
                MUTED,
            );
        }
        let pointer = self.pointer();
        let dragging = self.modal.is_none()
            && is_mouse_button_down(MouseButton::Left)
            && self
                .drag
                .as_ref()
                .is_some_and(|(start, _)| chart.contains(*start));
        let selected = self.click.filter(|p| chart.contains(*p)).or(if dragging {
            Some(pointer)
        } else {
            None
        });
        if let Some(p) = selected {
            self.review.as_mut().unwrap().ply =
                (((p.x - plot.x) / plot.w).clamp(0., 1.) * n as f32).round() as usize;
            self.click = None;
        }
    }
}
