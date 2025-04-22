fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("Failed to find vendored protoc");
    std::env::set_var("PROTOC", protoc);

    prost_build::compile_protos(&["resources/proto/lnrpc.proto"], &["resources/proto"])
        .expect("Failed to compile protos");
}
