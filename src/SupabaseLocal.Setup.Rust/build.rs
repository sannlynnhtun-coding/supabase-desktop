fn main() {
    println!("cargo:rerun-if-changed=../Shared/Branding/supabase.ico");
    println!("cargo:rerun-if-changed=branding.rc");
    embed_resource::compile("branding.rc", embed_resource::NONE)
        .manifest_required()
        .expect("Could not embed the original Supabase Windows icon");
}
