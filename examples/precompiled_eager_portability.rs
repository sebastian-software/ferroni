//! Run the same frozen native archive on native, WASI, and browser Wasm.
//! No clocks, filesystem, host imports, or unsafe exports are required.
#[path = "../tests/support/precompiled_eager.rs"]
mod checks;

fn main() {
    checks::check_fixture();
}
