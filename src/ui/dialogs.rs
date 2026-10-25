use super::*;
mod audio;
mod information;
mod records;
mod recovery;
mod rename;
mod setup;

impl App {
    pub(super) fn draw_modal(&mut self) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        if self.landscape() && matches!(modal, Modal::Audio) {
            self.wide_audio();
            return;
        }
        let old_canvas = (self.width, self.height);
        let compact_name = matches!(modal, Modal::Rename(..)) && self.viewport.h < 380.;
        let required_height: f32 = match &modal {
            Modal::Side(_) => 460.,
            Modal::End(_) => 540.,
            Modal::SaveFailed => 500.,
            Modal::StorageRecovery => 500.,
            Modal::Rename(..) => {
                if compact_name {
                    240.
                } else {
                    340.
                }
            }
            Modal::Delete(_) | Modal::Clear | Modal::ClearDrafts => 380.,
            _ => 685.,
        };
        let scale = self.height.min(self.viewport.h.max(required_height)) / self.height;
        if scale != 1. {
            self.width *= scale;
            self.height *= scale;
            self.click = self.click.map(|p| p * scale);
            if let Some((p, id)) = self.drag.take() {
                self.drag = Some((p * scale, id));
            }
            self.camera();
        }
        draw_rectangle(
            0.,
            0.,
            self.width,
            self.height,
            Color::new(0.12, 0.17, 0.15, 0.48),
        );
        let w = (self.width - 40.).min(480.);
        let x = (self.width - w) / 2.;
        let modal_height = match &modal {
            Modal::Side(_) => 380.,
            Modal::End(_) => 460.,
            Modal::SaveFailed => 420.,
            Modal::StorageRecovery => 420.,
            Modal::Rename(..) => {
                if compact_name {
                    212.
                } else {
                    292.
                }
            }
            Modal::Delete(_) | Modal::Clear | Modal::ClearDrafts => 300.,
            Modal::Drafts(_) => {
                let count = self.data.active.iter().count() + self.data.drafts.len();
                if count == 0 { 300. } else { 605. }
            }
            _ => 605.,
        };
        let y = (self.height - modal_height) / 2.;
        self.card(Rect::new(x, y, w, modal_height));
        match modal {
            Modal::ClearDrafts => self.modal_clear_drafts(x, y, w),
            Modal::Rename(id, edit) => self.modal_rename(id, edit, compact_name, x, y, w),
            Modal::Audio => self.modal_audio(x, y, w),
            Modal::Variation(text) => self.modal_variation(text, x, y, w),
            Modal::SaveFailed => self.modal_save_failed(x, y, w),
            Modal::StorageRecovery => self.modal_storage_recovery(x, y, w),
            Modal::Drafts(page) => self.modal_drafts(page, x, y, w),
            Modal::Difficulty => self.modal_difficulty(x, y, w),
            Modal::Side(board) => self.modal_side(board, x, y, w),
            Modal::End(score) => self.modal_end(score, x, y, w),
            Modal::Delete(id) => self.modal_delete(id, x, y, w),
            Modal::Clear => self.modal_clear(x, y, w),
            Modal::Help => self.modal_help(x, y, w),
            Modal::About => self.modal_about(x, y, w),
            Modal::License(index) => self.modal_license(index, x, y, w),
        }
        if scale != 1. {
            self.width = old_canvas.0;
            self.height = old_canvas.1;
            self.click = self.click.map(|p| p / scale);
            if let Some((p, id)) = self.drag.take() {
                self.drag = Some((p / scale, id));
            }
            self.camera();
        }
    }
}
