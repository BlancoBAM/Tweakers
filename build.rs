// Build script for Tweakers - compile Slint UI files
fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/theme.slint");
    // The slint::include_modules!() macro will find and compile these files
    slint_build::compile("ui/main.slint").unwrap();
}