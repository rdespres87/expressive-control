fn main() -> Result<(), slint_build::CompileError> {
    slint_build::compile("../firmwares/expressive-theremin/ui/theremin.slint")
}
