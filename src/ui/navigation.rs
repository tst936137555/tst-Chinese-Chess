use super::*;
impl App {
    pub(super) fn open_modal(&mut self, m: Modal) {
        self.redraw = true;
        self.accessible.clear();
        if self.name_keyboard {
            crate::platform::name_editor("", 0, 0, [0.; 4], false);
            macroquad::miniquad::window::show_keyboard(false);
            self.name_keyboard = false;
        }
        if matches!(m, Modal::Rename(..)) {
            if let Modal::Rename(id, _) = &m {
                self.rename_revision = self
                    .data
                    .active
                    .iter()
                    .chain(&self.data.drafts)
                    .chain(&self.data.archives)
                    .find(|r| r.id == *id)
                    .map_or(0, |r| r.title_revision);
            }
            self.rename_return = match &self.modal {
                Some(Modal::Drafts(page)) => Some(*page),
                _ => None,
            };
            while get_char_pressed().is_some() {}
            self.name_keyboard = true;
            macroquad::miniquad::window::set_ime_enabled(true);
            #[cfg(not(target_os = "ios"))]
            macroquad::miniquad::window::show_keyboard(true);
        }
        self.modal = Some(m);
        self.modal_scroll = 0.;
        self.modal_scroll_grab = None;
    }
    pub(super) fn finish_rename(&mut self) {
        self.accessible.clear();
        let _ = crate::text_input::take_native_edit();
        let _ = crate::text_input::take_native_done();
        crate::platform::name_editor("", 0, 0, [0.; 4], false);
        macroquad::miniquad::window::set_ime_enabled(false);
        macroquad::miniquad::window::show_keyboard(false);
        self.name_keyboard = false;
        self.modal = self.rename_return.take().map(Modal::Drafts);
    }
    pub(super) fn back(&mut self) {
        self.accessible.clear();
        self.cancel();
        self.engine_failed = false;
        self.notice.clear();
        self.modal = None;
        self.animation = None;
        self.save_review_cache();
        self.review = None;
        match self.navigation.pop().unwrap_or(PageState::Home) {
            PageState::Home => self.screen = Screen::Home,
            PageState::Archives(scroll) => {
                self.scroll = scroll;
                self.screen = Screen::Archives;
            }
            PageState::Game(page) => {
                self.game = page.game;
                self.settings.user_red = page.user_red;
                self.settings.level = page.level;
                self.preferences.user_red = page.user_red;
                self.preferences.level = page.level;
                self.review_game = page.review_game;
                self.end_ready = true;
                self.reset_move_view();
                self.screen = Screen::Game;
            }
            PageState::Review(mut review) => {
                self.settings.user_red = review.user_red;
                self.preferences.user_red = review.user_red;
                review.cache_loading = true;
                self.review = Some(*review);
                self.screen = Screen::Review;
                self.load_review_cache();
            }
        }
        self.ended_at = None;
    }
    pub(super) fn stash_page(&mut self) -> PageState {
        match self.screen {
            Screen::Home => PageState::Home,
            Screen::Archives => PageState::Archives(self.scroll),
            Screen::Game => PageState::Game(Box::new(SavedGamePage {
                game: self.game.clone(),
                user_red: self.settings.user_red,
                level: self.settings.level,
                review_game: self.review_game,
            })),
            Screen::Review => {
                PageState::Review(Box::new(self.review.take().expect("review page state")))
            }
        }
    }
    pub(super) fn start_game(&mut self, board: Option<Game>, red: bool) {
        self.accessible.clear();
        if self.data.drafts.len() >= crate::storage::MAX_DRAFTS && self.data.active.is_some() {
            self.notice = "对局已满，请先删除旧对局后开始新局".into();
            self.open_modal(Modal::Drafts(0));
            return;
        }
        if !self.saves.failed.is_empty() {
            self.notice = "请先重试保存，当前对局仍保留".into();
            self.open_modal(Modal::SaveFailed);
            return;
        }
        if self.screen != Screen::Game {
            let page = self.stash_page();
            self.navigation.push(page);
        }
        self.cancel();
        self.game = board.clone().unwrap_or_default();
        self.review_game = board.is_some();
        self.settings.user_red = red;
        if !self.review_game {
            self.save_preferences();
        } else {
            self.preferences.user_red = self.settings.user_red;
            self.preferences.level = self.settings.level;
        }
        self.engine_failed = false;
        self.notice.clear();
        self.animation = None;
        self.ended_at = None;
        self.end_ready = false;
        self.modal = None;
        self.reset_move_view();
        self.persist();
        self.screen = Screen::Game;
    }
    pub(super) fn resume(&mut self) {
        if !self.data.drafts.is_empty() {
            self.open_modal(Modal::Drafts(0));
        } else if let Some(r) = self.data.active.clone() {
            self.resume_record(r);
        }
    }
    pub(super) fn resume_record(&mut self, record: crate::game::Record) {
        self.accessible.clear();
        let record = self
            .saves
            .unsaved(&record.id)
            .filter(|failed| !record.acknowledges(failed))
            .cloned()
            .unwrap_or(record);
        self.load_record(record, false);
    }
    pub(super) fn load_record(&mut self, record: crate::game::Record, review: bool) {
        if self.loading_record.is_some() {
            return;
        }
        match self.loader.request(record, review) {
            Ok(id) => {
                self.loading_record = Some(id);
                self.notice = "正在读取棋谱…".into();
                self.redraw = true;
            }
            Err(error) => self.notice = error,
        }
    }
    pub(super) fn finish_resume(&mut self, record: crate::game::Record, game: Game) {
        self.cancel();
        self.navigation.clear();
        self.navigation.push(PageState::Home);
        self.settings.user_red = record.user_red;
        self.settings.level = LEVELS
            .iter()
            .position(|l| l.name == record.level_name)
            .unwrap_or(0);
        self.preferences.user_red = self.settings.user_red;
        self.preferences.level = self.settings.level;
        self.review_game = record.mode == "review";
        self.game = game;
        self.enqueue(Command::Activate(record.id));
        self.engine_failed = false;
        self.ended_at = None;
        self.end_ready = false;
        self.modal = None;
        self.reset_move_view();
        self.screen = Screen::Game;
    }
    pub(super) fn open_review(&mut self, game: Game, _origin: Screen) {
        self.accessible.clear();
        let page = self.stash_page();
        self.navigation.push(page);
        self.cancel();
        self.review = Some(Review::new(game, self.settings.user_red));
        self.engine_failed = false;
        self.screen = Screen::Review;
        self.load_review_cache();
        self.ended_at = None;
    }
    pub(super) fn change_level(&mut self, delta: isize) {
        self.settings.level = self.settings.level.saturating_add_signed(delta).min(4);
        self.save_preferences();
        self.persist();
    }
}
