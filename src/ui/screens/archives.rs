use super::*;
impl App {
    pub(in crate::ui) fn archives(&mut self) {
        let (x, w) = self.content();
        if self.archive_dirty {
            self.archive_order = (0..self.data.archives.len())
                .filter(|&i| !self.archive_favorites || self.data.archives[i].favorite)
                .filter(|&i| {
                    let r = &self.data.archives[i];
                    let query = self.archive_query.to_lowercase();
                    format!(
                        "{} {} {} {}",
                        r.title,
                        date_label(r.time),
                        r.result.label(),
                        r.level_name
                    )
                    .to_lowercase()
                    .contains(&query)
                })
                .collect();
            self.archive_order.sort_by_key(|&i| {
                let r = &self.data.archives[i];
                (!r.favorite, std::cmp::Reverse(r.updated_at))
            });
            self.archive_dirty = false;
        }
        self.text("每一局，都值得回看", x + 2., 116., 24., INK);
        if self.button(
            if self.archive_query.is_empty() {
                "查找"
            } else {
                "筛选中"
            },
            Rect::new(x + w - 108., 82., 108., 40.),
            false,
            true,
        ) {
            self.open_modal(Modal::Search(self.archive_query.clone().into()));
        }
        self.fit_text(
            &format!(
                "{} 局棋谱 · {} 局收藏",
                self.data.archives.len(),
                self.data.archives.iter().filter(|r| r.favorite).count()
            ),
            x + 2.,
            145.,
            w - 130.,
            15.,
            MUTED,
        );
        if self.button(
            "清空棋谱",
            Rect::new(x + w - 108., 130., 108., 40.),
            false,
            !self.data.archives.is_empty(),
        ) {
            self.open_modal(Modal::Clear);
        }
        if self.button(
            "全部棋谱",
            Rect::new(x, 176., (w - 10.) / 2., 42.),
            !self.archive_favorites,
            true,
        ) {
            self.archive_favorites = false;
            self.archive_dirty = true;
            self.scroll = 0.;
        }
        if self.button(
            "只看收藏",
            Rect::new(x + (w + 10.) / 2., 176., (w - 10.) / 2., 42.),
            self.archive_favorites,
            true,
        ) {
            self.archive_favorites = true;
            self.archive_dirty = true;
            self.scroll = 0.;
        }
        let viewport = Rect::new(x, 238., w, self.height - 272.);
        let plain = self.data.archives.iter().filter(|r| !r.favorite).count();
        if plain >= crate::storage::MAX_ARCHIVES.saturating_sub(5) {
            self.fit_text(
                &format!(
                    "普通棋谱 {plain}/{}，新棋谱将替换最旧记录",
                    crate::storage::MAX_ARCHIVES
                ),
                x + 2.,
                234.,
                w - 4.,
                12.,
                MUTED,
            );
        }
        let order = self.archive_order.clone();
        let row_step = 124.;
        let max = (order.len() as f32 * row_step - 12. - viewport.h).max(0.);
        if self.modal.is_none() && viewport.contains(self.pointer()) {
            self.scroll -= self.wheel().1 * 46.;
        }
        self.scroll = self.scroll.clamp(0., max);
        if is_mouse_button_down(MouseButton::Left)
            && self.modal.is_none()
            && let Some((start, _)) = self.drag
        {
            let pos = self.pointer();
            if (pos.y - start.y).abs() > 10. && viewport.contains(start) {
                self.scroll = (self.scroll + start.y - pos.y).clamp(0., max);
                self.drag = Some((pos, None));
                self.dragged = true;
            }
        }
        // A camera viewport clips partially visible rows without discarding their content.
        self.clip(viewport);
        let outside_click = self.click.filter(|p| !viewport.contains(*p));
        self.click = self.click.filter(|p| viewport.contains(*p));
        for (i, idx) in order.into_iter().enumerate() {
            let y = viewport.y + i as f32 * row_step - self.scroll;
            if y + 112. < viewport.y || y > viewport.bottom() {
                continue;
            }
            let r = self.data.archives[idx].clone();
            self.semantic_scope = format!("archive:{}", r.id);
            self.card(Rect::new(x, y, w, 112.));
            let title = if r.title.is_empty() {
                format!(
                    "{}{}",
                    if r.mode == "review" { "续下 · " } else { "" },
                    date_label(r.time)
                )
            } else {
                r.title.clone()
            };
            let top = y.max(viewport.y);
            let bottom = (y + 112.).min(viewport.bottom());
            self.semantic_named(
                "open",
                format!("复盘棋谱，{}，{}", title, r.result.label()),
                Rect::new(x, top, w - 150., (bottom - top).max(1.)),
                true,
            );
            self.fit_text(&title, x + 16., y + 29., w - 110., 19., INK);
            self.fit_text(
                &if r.title.is_empty() {
                    r.result.label().to_owned()
                } else {
                    format!("{} · {}", r.result.label(), date_label(r.time))
                },
                x + 16.,
                y + 59.,
                w - 32.,
                16.,
                if r.result == Outcome::Draw {
                    MUTED
                } else {
                    JADE
                },
            );
            self.fit_text(
                &format!(
                    "{} · {} · {} 步",
                    if r.user_red { "执红" } else { "执黑" },
                    r.level_name,
                    r.history.len()
                ),
                x + 16.,
                y + 88.,
                w - 158.,
                15.,
                MUTED,
            );
            if self.button_named(
                "favorite",
                if r.favorite { "\u{2605}" } else { "\u{2606}" },
                Rect::new(x + w - 66., y + 10., 48., 42.),
                false,
                true,
            ) {
                self.enqueue(Command::SetFavorite {
                    id: r.id.clone(),
                    favorite: !r.favorite,
                    revision: r.favorite_revision,
                });
            }
            if self.button(
                "命名",
                Rect::new(x + w - 140., y + 62., 56., 38.),
                false,
                true,
            ) {
                self.open_modal(Modal::Rename(r.id.clone(), r.title.clone().into()));
            }
            if self.button(
                "删除",
                Rect::new(x + w - 76., y + 62., 60., 38.),
                false,
                true,
            ) {
                self.open_modal(Modal::Delete(r.id.clone()));
            }
            if self
                .click
                .is_some_and(|p| Rect::new(x, y, w - 150., 112.).contains(p))
            {
                self.click = None;
                self.load_record(r, true);
                break;
            }
            if let Some((start, _)) = self.drag.clone()
                && self.modal.is_none()
                && is_mouse_button_released(MouseButton::Left)
            {
                let end = self.pointer();
                if Rect::new(x, y, w, 112.).contains(start)
                    && end.x - start.x < -60.
                    && (end.y - start.y).abs() < 35.
                {
                    self.open_modal(Modal::Delete(r.id.clone()));
                }
            }
        }
        self.click = self.click.or(outside_click);
        self.semantic_scope.clear();
        self.camera();
        if self.archive_order.is_empty() {
            let cy = viewport.y + viewport.h * 0.40;
            self.card(Rect::new(x, cy - 94., w, 232.));
            self.center(
                if self.archive_favorites {
                    "还没有收藏的棋谱"
                } else {
                    "棋谱，等你落笔"
                },
                x + w / 2.,
                cy - 38.,
                25.,
                INK,
            );
            self.center(
                if self.archive_favorites {
                    "点击棋谱右侧的星标即可收藏"
                } else {
                    "完成对局后，棋谱会自动保存于此"
                },
                x + w / 2.,
                cy + 3.,
                16.,
                MUTED,
            );
            if self.button(
                if self.archive_favorites {
                    "查看全部棋谱"
                } else {
                    "开始一局"
                },
                Rect::new(x + 28., cy + 43., w - 56., 52.),
                true,
                true,
            ) {
                if self.archive_favorites {
                    self.archive_favorites = false;
                    self.archive_dirty = true;
                } else {
                    self.open_modal(Modal::Difficulty);
                }
            }
        }
        if max > 0. {
            let h = (viewport.h * viewport.h / (max + viewport.h)).max(24.);
            self.panel(
                Rect::new(
                    x + w - 4.,
                    viewport.y + self.scroll / max * (viewport.h - h),
                    3.,
                    h,
                ),
                GOLD,
            );
        }
    }
}
