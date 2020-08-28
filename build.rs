include!("build/config.rs");

fn main() {
    config::Cfgs::new().apply();
}
