use super::*;
impl App {
    pub(in crate::ui) fn end_overlay(&mut self) {
        if !self.end_ready
            && matches!(
                self.game.end_reason,
                Some(crate::game::EndReason::Checkmate)
            )
        {
            let ended_at = *self.ended_at.get_or_insert_with(get_time);
            if get_time() - ended_at < 1. {
                return;
            }
        }
        self.end_ready = true;
        draw_rectangle(
            0.,
            0.,
            self.width,
            self.height,
            Color::new(0.12, 0.17, 0.15, 0.55),
        );
        let w = (self.width - 40.).min(460.);
        let h = 456.;
        let x = (self.width - w) / 2.;
        let y = (self.height - h) / 2.;
        self.card(Rect::new(x, y, w, h));
        let title = if self.game.outcome == Outcome::Draw {
            "握手言和"
        } else if self.game.outcome == Outcome::winner(self.player()) {
            "此局获胜"
        } else {
            "胜败之间，皆有所得"
        };
        self.center(
            title,
            self.width / 2.,
            y + 55.,
            30.,
            if self.game.outcome == Outcome::winner(self.player()) {
                JADE
            } else {
                INK
            },
        );
        self.fit_text(&self.game.reason, x + 24., y + 102., w - 48., 17., MUTED);
        self.panel(Rect::new(x + 24., y + 129., w - 48., 60.), PAPER);
        self.center(
            &format!(
                "{} 步 · {}",
                self.game.history.len(),
                LEVELS[self.settings.level].name
            ),
            self.width / 2.,
            y + 159.,
            17.,
            JADE,
        );
        if self.button(
            "复盘此局",
            Rect::new(x + 24., y + 216., w - 48., 54.),
            true,
            !self.game.history.is_empty(),
        ) {
            self.open_review(self.game.clone(), Screen::Game);
        }
        if !self.review_game
            && self.button(
                "再来一局",
                Rect::new(x + 24., y + 287., w - 48., 50.),
                false,
                true,
            )
        {
            self.start_game(None, self.settings.user_red);
        }
        if self.button(
            if self.review_game {
                "返回复盘"
            } else {
                "返回首页"
            },
            Rect::new(x + 24., y + 358., w - 48., 50.),
            false,
            true,
        ) {
            self.back();
        }
    }
}
