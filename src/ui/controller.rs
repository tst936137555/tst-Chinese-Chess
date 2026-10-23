use super::*;
impl App {
    pub(super) fn recover_branches(&mut self) {
        let failed = std::mem::take(&mut self.saves.failed);
        for command in failed {
            let Command::Save(record, settings) = command else {
                self.saves.failed.push(command);
                continue;
            };
            let original = record.id.clone();
            let current = self.game.snapshot(
                self.settings.user_red,
                self.settings.level,
                self.review_game,
            );
            for source in crate::session::recovery_records(&record, Some(&current)) {
                match source.replay() {
                    Ok(mut game) => {
                        game.fork_identity();
                        let level = LEVELS
                            .iter()
                            .position(|l| l.name == source.level_name)
                            .unwrap_or(0);
                        let mut branch =
                            game.snapshot(source.user_red, level, source.mode == "review");
                        branch.title = if source.title.is_empty() {
                            self.data
                                .active
                                .iter()
                                .chain(&self.data.drafts)
                                .chain(&self.data.archives)
                                .find(|r| r.id == source.id)
                                .map_or_else(String::new, |r| r.title.clone())
                        } else {
                            source.title.clone()
                        };
                        if self.game.id == original && source.acknowledges(&current) {
                            self.game = game;
                            self.reset_move_view();
                        }
                        self.enqueue(Command::save(branch, settings.clone()));
                    }
                    Err(e) => {
                        self.saves.failed(Command::save(source, settings.clone()));
                        self.notice = e;
                    }
                }
            }
        }
        if self.notice.is_empty() {
            self.notice = "正在另存独立分支".into();
        }
    }
    pub(super) fn write_test_state(&mut self) {
        let Some(path) = crate::platform::observation_path() else {
            return;
        };
        if get_time() - self.observed_at < 0.1 {
            return;
        }
        self.observed_at = get_time();
        let screen = match self.screen {
            Screen::Home => "home",
            Screen::Game => "game",
            Screen::Archives => "archives",
            Screen::Review => "review",
        };
        let controls: Vec<_> = self
            .observed_controls
            .iter()
            .map(|(name, r)| serde_json::json!({"label":name,"x":r.x,"y":r.y,"w":r.w,"h":r.h}))
            .collect();
        let (origin, cell) = self.board_rect();
        let unit = self.width / self.viewport.w;
        let squares:Vec<_>=(0..10).flat_map(|rank|(0..9).map(move|file|(file,rank))).map(|(file,rank)|{let square=Square::new(file,rank).unwrap();let point=Self::point(origin,cell,square,!self.settings.user_red);serde_json::json!({"file":file,"rank":rank,"x":self.viewport.x+point.x/unit,"y":self.viewport.y+point.y/unit})}).collect();
        let value = serde_json::json!({"screen":screen,"modal":self.modal.is_some(),"fen":self.game.board.fen(),"plies":self.game.history.len(),"pending":self.pending.is_some(),"pauseSerial":self.pause_serial,"saved":self.saves.pending==0&&self.saves.failed.is_empty(),"reviewPly":self.review.as_ref().map(|r|r.ply),"controls":controls,"squares":squares,"deviceScale":screen_dpi_scale(),"viewOrigin":crate::platform::view_origin(),"screenWidth":screen_width(),"screenHeight":screen_height()});
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(&value) {
            let _ = std::fs::write(path, bytes);
        }
    }
    pub(super) fn enqueue(&mut self, command: Command) {
        let data_command = command.changes_data();
        match self.io.send(command.clone()) {
            Ok(seq) => {
                if data_command {
                    self.saves.queued(seq, command);
                }
            }
            Err(e) => {
                self.notice = e;
                if data_command {
                    self.saves.failed.push(command);
                }
            }
        }
    }
    pub(super) fn cancel(&mut self) {
        self.board_cursor = None;
        self.recovery.resume = None;
        self.engine.cancel();
        self.pending = None;
        self.selected = None;
        self.hints.clear();
        if let Some(r) = &mut self.review {
            r.analyzing = false;
        }
    }
    pub(super) fn reload(&mut self) {
        self.enqueue(Command::Load);
    }
    pub(super) fn persist(&mut self) {
        let mut record = self.game.snapshot(
            self.settings.user_red,
            self.settings.level,
            self.review_game,
        );
        if let Some(old) = self
            .data
            .active
            .iter()
            .chain(&self.data.drafts)
            .chain(&self.data.archives)
            .find(|r| r.id == record.id)
        {
            record.title = old.title.clone();
            record.favorite = old.favorite;
        }
        if record.result == Outcome::Playing {
            if let Some(i) = self.data.drafts.iter().position(|r| r.id == record.id) {
                self.data.drafts[i] = record.clone();
            } else {
                if self.data.active.as_ref().is_some_and(|r| r.id != record.id) {
                    self.data.drafts.push(self.data.active.take().unwrap());
                }
                self.data.active = Some(record.clone());
            }
        } else {
            if self.data.active.as_ref().is_some_and(|r| r.id == record.id) {
                self.data.active = None;
            }
            self.data.drafts.retain(|r| r.id != record.id);
        }
        let settings = if self.review_game {
            None
        } else {
            Some(self.settings.clone())
        };
        self.enqueue(Command::save(record, settings));
        self.move_layout_key.1 = u64::MAX;
        self.legal_key.clear();
    }
    pub(super) fn reset_move_view(&mut self) {
        self.move_scroll = 0.;
        self.move_follow = true;
        self.move_layout_key = (String::new(), u64::MAX);
        self.legal_key.clear();
    }
    pub(super) fn sound(&mut self, index: usize) {
        let file = self.game.history.last().map_or(4, |e| e.mv.to.file());
        let visible = if self.settings.user_red {
            file
        } else {
            8 - file
        };
        self.audio.play(index, visible, &self.settings, get_time());
    }
    pub(super) fn play(&mut self, m: Move) {
        let animation = self.game.board.at(m.from).map(|piece| Animation {
            mv: m,
            piece,
            captured: self.game.board.at(m.to),
            started: get_time(),
        });
        match self.game.play(m) {
            Ok(()) => {
                self.animation = animation;
                self.selected = None;
                self.hints.clear();
                let e = self.game.history.last().unwrap();
                let index = if self.game.outcome != Outcome::Playing {
                    if self.game.outcome == Outcome::Draw {
                        5
                    } else if self.game.outcome == Outcome::winner(self.player()) {
                        3
                    } else {
                        4
                    }
                } else if e.check {
                    2
                } else if e.capture {
                    1
                } else {
                    0
                };
                self.sound(index);
                self.persist();
            }
            Err(e) => self.notice = e,
        }
    }
    pub(super) fn finish_resign(&mut self) {
        self.cancel();
        self.game.resign(self.player());
        self.modal = None;
        self.animation = None;
        self.sound(4);
        self.persist();
    }
    pub(super) fn search(&mut self, job: Job) {
        if self.pending.is_some() || crate::platform::suspended() {
            return;
        }
        let request = match &job {
            Job::Move => {
                let l = LEVELS[self.settings.level];
                self.engine
                    .request_game(&self.game, self.game.history.len(), l.depth, l.ms, l.pv)
            }
            Job::Hint => self
                .engine
                .request_game(&self.game, self.game.history.len(), 12, 3000, 2),
            Job::End => self
                .engine
                .request_game(&self.game, self.game.history.len(), 12, 3000, 1),
            Job::Review(ply) => {
                let Some(r) = &self.review else {
                    return;
                };
                let (depth, ms) = crate::review::ANALYSIS_LIMITS;
                self.engine.request_game(&r.game, *ply, depth, ms, 1)
            }
        };
        match request {
            Ok(id) => self.pending = Some(Pending { id, job }),
            Err(e) => {
                self.notice = e;
                self.engine_failed = true;
            }
        }
    }
    pub(super) fn load_review_cache(&mut self) {
        if let Some(r) = &self.review {
            self.enqueue(Command::CacheLoad {
                key: r.cache_key.clone(),
            });
        }
    }
    pub(super) fn save_review_cache(&mut self) {
        if let Some(r) = &self.review {
            let entries = r
                .scores
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    s.map(|score| CachedPosition {
                        score,
                        best: r.bests[i].map(|m| m.to_string()),
                        depth: r.depths[i],
                    })
                })
                .collect();
            self.enqueue(Command::CachePut {
                key: r.cache_key.clone(),
                entries,
            });
        }
    }
    pub(super) fn poll(&mut self) {
        while let Ok(event) = self.io.rx.try_recv() {
            match event.result {
                Ok(Response::Settings(settings)) => {
                    self.data.settings = settings;
                    self.saves.succeeded(event.seq, &self.data);
                }
                Ok(Response::Data(data)) => {
                    self.saves.succeeded(event.seq, &data);
                    if matches!(self.modal, Some(Modal::StorageRecovery)) {
                        self.modal = None;
                        self.settings = data.settings.clone();
                        self.notice = "存档已读取".into();
                    }
                    self.data = self.saves.project(*data);
                    self.archive_dirty = true;
                }
                Err(e) => {
                    if event.retry.is_some() {
                        self.saves.completed(event.seq);
                        if let Some(command) = event.retry {
                            self.saves.failed(command);
                        }
                    }
                    self.notice = format!("保存或读取失败：{e}（当前棋盘已保留）");
                    if self.closing {
                        self.closing = false;
                        self.open_modal(Modal::SaveFailed);
                    }
                    // Cache errors must never strand the viewer waiting for an event.
                    if let Some(r) = &mut self.review {
                        r.cache_loading = false;
                        r.score_version = r.score_version.wrapping_add(1);
                    }
                }
                Ok(Response::Cache { key, review }) => {
                    if let Some(r) = &mut self.review
                        && key == r.cache_key
                    {
                        r.cache_loaded(review);
                    }
                }
                Ok(Response::CacheWarning(message)) => {
                    crate::diagnostics::failure(
                        crate::diagnostics::FailureKind::Storage,
                        "cache",
                        &message,
                    );
                    self.notice = format!("分析缓存未保存：{message}；主存档不受影响");
                }
                Ok(Response::CachePersisted) => {}
                Ok(Response::Flushed) => self.close_flushed = true,
                Ok(Response::SavedCache) => {}
                Ok(Response::Superseded) => {
                    self.saves.completed(event.seq);
                }
            }
        }
        while let Ok(reply) = self.engine.rx.try_recv() {
            if self.pending.as_ref().is_none_or(|p| p.id != reply.id) {
                continue;
            }
            let pending = self.pending.take().unwrap();
            match reply.result {
                Err(e) => {
                    self.notice = if matches!(
                        reply.failure_kind,
                        Some(crate::diagnostics::FailureKind::RestartRequired)
                    ) {
                        format!("引擎已停止：{e}。请保存对局后重启应用")
                    } else {
                        format!("引擎不可用：{e}（可重试或认输）")
                    };
                    self.engine_failed = true;
                    if let Some(r) = &mut self.review {
                        r.analyzing = false;
                    }
                }
                Ok(analysis) => match pending.job {
                    Job::Move => {
                        let m = self.choose(&analysis);
                        self.play(m);
                    }
                    Job::Hint => {
                        self.hints = analysis.variations.iter().take(2).map(|v| v.mv).collect();
                        self.notice = format!(
                            "建议：{}",
                            self.hints
                                .iter()
                                .map(|m| crate::notation::chinese(&self.game.board, *m)
                                    .unwrap_or_else(|_| m.to_string()))
                                .collect::<Vec<_>>()
                                .join(" / ")
                        );
                    }
                    Job::End => {
                        if let Some(v) = analysis.variations.iter().find(|v| v.mv == analysis.best)
                            && matches!(self.modal, Some(Modal::End(_)))
                        {
                            self.modal = Some(Modal::End(Some(crate::game::EvaluationEvidence {
                                score: v.evaluation,
                                requested_depth: analysis.requested_depth,
                                reached_depth: v.depth,
                                budget_ms: analysis.budget_ms,
                                elapsed_ms: analysis.elapsed_ms,
                                assets: analysis.assets.clone(),
                            })));
                        }
                    }
                    Job::Review(ply) => {
                        if let Some(r) = &mut self.review
                            && let Some(v) =
                                analysis.variations.iter().find(|v| v.mv == analysis.best)
                        {
                            r.analyzed(ply, v);
                        }
                        self.save_review_cache();
                    }
                },
            }
        }
        if self.pending.is_none() && !self.engine_failed && !crate::platform::suspended() {
            let mut next = None;
            if let Some(r) = &mut self.review
                && r.analyzing
                && !r.cache_loading
            {
                if let Some(i) = crate::review::next_missing(
                    r.ply,
                    &r.scores.iter().map(Option::is_some).collect::<Vec<_>>(),
                ) {
                    if let Some(score) = crate::review::rule_score(&r.game, i) {
                        r.scores[i] = Some(score);
                        r.score_version = r.score_version.wrapping_add(1);
                    } else {
                        next = Some(i);
                    }
                } else {
                    r.analyzing = false;
                }
            }
            if let Some(i) = next {
                self.search(Job::Review(i));
            }
        }
    }
    pub(super) fn handle_lifecycle(&mut self) {
        let serial = crate::platform::pause_serial();
        if serial != self.pause_serial {
            self.pause_serial = serial;
            let kind = self
                .pending
                .as_ref()
                .map(|p| match p.job {
                    Job::Move => crate::session::TaskKind::Move,
                    Job::Hint => crate::session::TaskKind::Hint,
                    Job::Review(_) => crate::session::TaskKind::Review,
                    Job::End => crate::session::TaskKind::End,
                })
                .or_else(|| {
                    self.review
                        .as_ref()
                        .filter(|r| r.analyzing)
                        .map(|_| crate::session::TaskKind::Review)
                });
            let kind = kind.or(self.recovery.resume);
            self.save_review_cache();
            self.cancel();
            self.recovery.pause(kind);
            self.animation = None;
            self.drag = None;
            self.click = None;
            self.enqueue(Command::Flush);
            self.audio.suspend();
        }
        if let Some(kind) = self.recovery.resume(crate::platform::suspended()) {
            self.engine_failed = false;
            match kind {
                crate::session::TaskKind::Review => {
                    if let Some(r) = &mut self.review {
                        r.analyzing = true;
                    }
                }
                crate::session::TaskKind::End if matches!(self.modal, Some(Modal::End(None))) => {
                    self.search(Job::End)
                }
                crate::session::TaskKind::Hint
                    if self.screen == Screen::Game && self.game.outcome == Outcome::Playing =>
                {
                    self.search(Job::Hint)
                }
                _ => {}
            }
        }
    }

    pub(super) fn landscape(&self) -> bool {
        self.height < 800. && self.width / self.height >= 1.35
    }
    pub(super) fn legal_moves(&mut self, board: &Board) -> Vec<Move> {
        let key = board.position_key();
        if key != self.legal_key {
            self.legal_cache = board.legal_moves();
            self.legal_key = key;
        }
        self.legal_cache.clone()
    }
    pub(super) fn choose(&self, a: &Analysis) -> Move {
        crate::engine::choose_variation(
            a,
            self.game.board.turn,
            LEVELS[self.settings.level].tolerance,
            self.game.random_seed,
            self.game.history.len(),
        )
    }
}

impl App {
    pub(super) fn pointer(&self) -> Vec2 {
        let point = crate::interaction::map_point(
            [mouse_position().0, mouse_position().1],
            [
                self.viewport.x,
                self.viewport.y,
                self.viewport.w,
                self.viewport.h,
            ],
            [self.width, self.height],
        );
        vec2(point[0], point[1])
    }

    pub(super) fn camera(&self) {
        let mut camera = Camera2D::from_display_rect(Rect::new(0., 0., self.width, self.height));
        camera.render_target = Some(self.scene.target.clone());
        let scale = self.scene.scale();
        self.font
            .set_pixel_density(scale.y * self.viewport.h / self.height);
        camera.viewport = Some((
            (self.viewport.x * scale.x).round() as i32,
            ((screen_height() - self.viewport.y - self.viewport.h) * scale.y).round() as i32,
            (self.viewport.w * scale.x).round() as i32,
            (self.viewport.h * scale.y).round() as i32,
        ));
        set_camera(&camera);
        self.scene.use_scene_material();
    }
    pub(super) fn clip(&self, rect: Rect) {
        let mut camera = Camera2D::from_display_rect(rect);
        camera.render_target = Some(self.scene.target.clone());
        let scale = self.scene.scale();
        let sx = self.viewport.w / self.width;
        let sy = self.viewport.h / self.height;
        camera.viewport = Some((
            ((self.viewport.x + rect.x * sx) * scale.x).round() as i32,
            ((screen_height() - self.viewport.y - rect.bottom() * sy) * scale.y).round() as i32,
            (rect.w * sx * scale.x).round() as i32,
            (rect.h * sy * scale.y).round() as i32,
        ));
        set_camera(&camera);
        self.scene.use_scene_material();
    }
}
