fn main() {
    embuild::espidf::sysenv::output();
    slint_build::compile("ui/theremin.slint").unwrap();
}
