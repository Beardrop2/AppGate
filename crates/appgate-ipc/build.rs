fn main() {
    // Ensure changes to protos trigger rebuilds
    println!("cargo:rerun-if-changed=proto/pdp.proto");
    println!("cargo:rerun-if-changed=proto");

    // Use a vendored protoc, avoiding system dependency
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate vendored protoc");
    std::env::set_var("PROTOC", protoc);

    tonic_build::configure()
        // add options as needed, e.g. .build_server(true).build_client(true)
        .compile(&["proto/pdp.proto"], &["proto"])
        .expect("failed to compile protos");
}
