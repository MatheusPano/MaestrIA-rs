mod banner;
mod center;
mod row;
mod toasts;

pub use banner::Banner;
pub use center::{NoticeCenter, NoticeCenterProps};
#[cfg(test)]
pub use center::cascade_total;
pub use row::{NoticeView, RowHandlers};
#[cfg(test)]
pub use row::{meta, sentence};
pub use toasts::ToastStack;

pub fn anim_off() -> bool {
    std::env::var_os("MAESTRIA_REDUCE_MOTION").is_some()
}

#[cfg(test)]
pub mod toasts_frames {
    pub use super::toasts::enter_frame;
}
