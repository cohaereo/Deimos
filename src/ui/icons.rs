macro_rules! include_icon {
    ($name:ident, $filename:expr) => {
        pub const $name: egui::ImageSource<'static> =
            egui::include_image!(concat!("../../assets/ui/icons/", $filename));
    };
}

include_icon!(LIGHT, "light.png");
include_icon!(UNKNOWN, "unknown.png");
