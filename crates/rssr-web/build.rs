// The build identifier the API checks on writes (`build-support/build_id.rs`).
include!("../../build-support/build_id.rs");

fn main() {
    emit_build_id();
}
