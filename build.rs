fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(&["resources/proto/lnrpc.proto"], &["resources/proto"])
        .expect("Failed to compile protos");
}
