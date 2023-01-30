fn main() -> Result<(), std::io::Error> {
    let builder = tonic_build::configure().build_server(false);
    builder.compile(&["proto/auth.proto", "proto/user.proto"], &["proto"])
}
