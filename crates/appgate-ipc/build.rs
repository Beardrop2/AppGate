fn main() {
    tonic_build::configure()
        .compile(&["proto/pdp.proto"], &["proto"])
        .unwrap();
}