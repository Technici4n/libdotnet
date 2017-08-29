extern crate libloading as lib;
extern crate libc;

extern crate libdotnet;

const MONO_FRAMEWORK_ETC: &'static str = "/Library/Frameworks/Mono.framework/Versions/Current/etc/";
const MONO_FRAMEWORK_LIB: &'static str = "/Library/Frameworks/Mono.framework/Versions/Current/lib/";

fn main() {
    let mut args = std::env::args();

    match (args.next(), args.next()) {
        (_, Some(exe_path)) => {
            let runtime = libdotnet::rt::Runtime::init(MONO_FRAMEWORK_ETC, MONO_FRAMEWORK_LIB, "DomainFromRust").expect("failed to initialize runtime");
            let asm = runtime.open_assembly(exe_path).expect("failed to load the assembly");
            let ret_val = runtime.execute(&asm).expect("encountered an unknown error in managed land");
            println!("Exit code from managed land: {}", ret_val);
        },
        (Some(invocation_path), _) => println!("Usage: {} <path/to/assembly.exe>", invocation_path),
        _ => unreachable!(),
    }
}