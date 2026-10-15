use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=SUPABASE_PAYLOAD");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("payload.rs");
    let code = match env::var("SUPABASE_PAYLOAD") {
        Ok(path) => {
            let path = fs::canonicalize(path).expect("SUPABASE_PAYLOAD must exist");
            println!("cargo:rerun-if-changed={}", path.display());
            format!("pub static PAYLOAD: &[u8] = include_bytes!({:?});", path)
        }
        Err(_) => "pub static PAYLOAD: &[u8] = &[];".into(),
    };
    fs::write(out, code).unwrap();
}
