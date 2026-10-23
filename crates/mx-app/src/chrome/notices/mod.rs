mod banner;
mod center;
mod row;
mod toasts;

pub use banner::{BANNER_LIFE, Banner};
pub use center::{EMPTY, NoticeCenter, NoticeCenterProps, cascade_total};
pub use row::{NoticeView, RowHandlers, kind_color, kind_icon, meta, notice_row, sentence};
pub use toasts::{TOAST_ENTER, TOAST_LIFE, ToastStack};

pub mod toasts_frames {
    pub use super::toasts::{enter_frame, exit_frame};
}

pub fn anim_off() -> bool {
    std::env::var_os("MAESTRIA_REDUCE_MOTION").is_some()
}
