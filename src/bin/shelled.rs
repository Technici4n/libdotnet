extern crate libloading as lib;
extern crate libc;

extern crate libdotnet;
use libdotnet::rt::{ShelledRuntime, ShelledRuntimeError};

const LIBMONO_DYLIB_PATH: &'static str = "/Library/Frameworks/Mono.framework/Versions/Current/lib/libmono-2.0.dylib";
const _MSBUILD_DLL_PATH: &'static str = "/Library/Frameworks/Mono.framework/Versions/Current/lib/mono/msbuild/15.0/bin/MSBuild.dll";

fn main() {
    let mut args = std::env::args();

    match (args.next(), args.next()) {
        (_, Some(asm_path)) => {
            match ShelledRuntime::run(LIBMONO_DYLIB_PATH, asm_path, args) {
                Ok(()) => {},
                Err(ShelledRuntimeError::FailedToLoadLibMono) => unimplemented!(),
                Err(ShelledRuntimeError::FailedToFindMonoMainSymbol) => unimplemented!(),
                Err(ShelledRuntimeError::ArgumentContainsNulByte(_)) => unimplemented!(),
                Err(ShelledRuntimeError::PathContainsNulByte(_)) => unimplemented!(),
                Err(ShelledRuntimeError::NonZeroExitCode(code)) => println!("*** Runtime exited with code: {} ***", code),
                _ => unreachable!(),
            }
        },
        (Some(invocation_path), _) => println!("Usage: {} <path/to/assembly.exe>", invocation_path),
        _ => unreachable!(),
    }
}