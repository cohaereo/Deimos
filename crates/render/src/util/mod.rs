pub mod byteutil;
pub mod filtering;
pub mod fps_histogram;
pub mod geometry;
pub mod math;
pub mod range;
pub mod thread_cell;

macro_rules! once {
    ($($body:stmt)*) => {{
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            $($body)*
        });
    }};
}

pub(crate) use once;
