fn main() {
    // Las traducciones (.po) se incrustan en el binario: no hay archivos sueltos que instalar.
    let i18n = concat!(env!("CARGO_MANIFEST_DIR"), "/../../i18n");
    println!("cargo:rerun-if-changed={i18n}");
    let config = slint_build::CompilerConfiguration::new().with_bundled_translations(i18n);
    slint_build::compile_with_config("ui/app.slint", config)
        .expect("no se pudo compilar la interfaz");
}
