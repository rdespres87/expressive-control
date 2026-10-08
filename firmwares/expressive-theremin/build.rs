fn main() {
    embuild::espidf::sysenv::output();
    slint_build::compile("ui/index.slint").unwrap();
}
