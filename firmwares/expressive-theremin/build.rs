fn main() {
    embuild::espidf::sysenv::output();
    println!("cargo:rustc-link-lib=static=__idf_espressif__esp_tinyusb");
    println!("cargo:rustc-link-lib=static=__idf_espressif__tinyusb");
}
