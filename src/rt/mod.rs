use std::path::{Path, PathBuf};
use std::ffi::{CString, NulError};
use libc::{c_char, c_int};
use lib;

// TODO: This prevents libdotnet from compiling on Windows.
use std::os::unix::ffi::OsStringExt;

pub type RtResult<T> = Result<T, RtError>;

#[derive(Debug)]
pub enum RtError {
    DataContainsNulByte,
    InvalidAssemblyPath,
    RetValWasNull,
}

impl From<NulError> for RtError {
    fn from(_: NulError) -> Self {
        RtError::DataContainsNulByte
    }
}

mod raw {
    #[repr(u8)]
    pub enum MonoDomain {
        #[allow(non_camel_case_types)]
        __variant1,
        #[allow(non_camel_case_types)]
        __variant2,
    }
    #[repr(u8)]
    pub enum MonoAssembly {
        #[allow(non_camel_case_types)]
        __variant1,
        #[allow(non_camel_case_types)]
        __variant2,
    }
}

#[allow(non_camel_case_types)]
type t_mono_main = fn(c_int, *const *const c_char) -> c_int;
type SymMain<'sym> = lib::Symbol<'sym, t_mono_main>;

#[allow(non_camel_case_types)]
type t_mono_set_dirs = fn(*const c_char, *const c_char);

#[allow(non_camel_case_types)]
type t_mono_jit_init = fn(*const c_char) -> *mut raw::MonoDomain;

#[allow(non_camel_case_types)]
type t_mono_domain_assembly_open = fn(*mut raw::MonoDomain, *const c_char) -> *mut raw::MonoAssembly;

#[allow(non_camel_case_types)]
type t_mono_jit_exec = fn(*mut raw::MonoDomain, *mut raw::MonoAssembly, c_int, *const *const c_char) -> c_int;

pub struct MonoDomain {
    raw: *mut raw::MonoDomain,
}

pub struct MonoAssembly {
    raw: *mut raw::MonoAssembly,
}

fn get_sym<'lib, FnT>(lib: &'lib lib::Library, name: &str) -> ::std::io::Result<lib::Symbol<'lib, FnT>> {
    unsafe { lib.get(name.as_bytes()) }
}

#[derive(Debug)]
pub enum InitError {
    InvalidPath(PathBuf),
    FailedToLoadLibrary(::std::io::Error),
    FailedToFindSymbol(String),
    InvalidDomainName(String),
    FailedToCreateDomain,

    #[doc(hidden)]
    __NonExhaustive,
}

#[derive(Debug)]
pub enum ShelledRuntimeError {
    FailedToLoadLibMono,
    FailedToFindMonoMainSymbol,
    ArgumentContainsNulByte(usize),
    PathContainsNulByte(usize),
    NonZeroExitCode(usize),

    #[doc(hidden)]
    __NonExhaustive,
}

pub struct ShelledRuntime;

impl ShelledRuntime {
    pub fn run<P1, P2, T>(libmono_dylib_path: P1, assembly_path: P2, args: T) -> Result<(), ShelledRuntimeError>
        where P1: Into<PathBuf>,
              P2: Into<PathBuf>,
               T: IntoIterator,
               T::Item: AsRef<str> {
        let libmono_dylib_path = libmono_dylib_path.into();
        let assembly_path = assembly_path.into();

        let lib_mono = lib::Library::new(&libmono_dylib_path).map_err(|_| ShelledRuntimeError::FailedToLoadLibMono)?;
        let sym_main: SymMain = unsafe { lib_mono.get(b"mono_main") }.map_err(|_| ShelledRuntimeError::FailedToFindMonoMainSymbol)?;

        let os_libmono_path = libmono_dylib_path.into_os_string().into_vec();
        let c_invocation_path = CString::new(os_libmono_path).map_err(|e| ShelledRuntimeError::PathContainsNulByte(e.nul_position()))?;

        let os_assembly_path = assembly_path.into_os_string().into_vec();
        let c_asm_path = CString::new(os_assembly_path).map_err(|e| ShelledRuntimeError::PathContainsNulByte(e.nul_position()))?;

        let c_args = args.into_iter().map(|arg| CString::new(arg.as_ref()).map_err(|e| ShelledRuntimeError::ArgumentContainsNulByte(e.nul_position()))).collect::<Result<Vec<_>, _>>()?;
        let mut args = vec![&c_invocation_path, &c_asm_path];
        args.extend(c_args.iter());

        let raw_args = args.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();

        match sym_main(raw_args.len() as c_int, raw_args.as_ptr()) as usize {
            0 => Ok(()),
            anything_else @ _ => Err(ShelledRuntimeError::NonZeroExitCode(anything_else)),
        }
    }
}

/// An instance of the Mono runtime.
pub struct Runtime<'rt> {
    pub etc_path: PathBuf,
    pub lib_path: PathBuf,
    pub domain: MonoDomain,

    _lib: lib::Library,
    _sym_asm_open: lib::Symbol<'rt, t_mono_domain_assembly_open>,
    _sym_jit_execute: lib::Symbol<'rt, t_mono_jit_exec>,
}

impl<'rt> Runtime<'rt> {
    pub fn init<P1, P2>(etc_path: P1, lib_path: P2, domain_name: &str) -> Result<Self, InitError>
        where P1: Into<PathBuf>,
              P2: Into<PathBuf> {
        let etc_path = etc_path.into();
        let lib_path = lib_path.into();

        let c_etc_path = {
            let etc_path_str = etc_path.to_str().ok_or_else(|| InitError::InvalidPath(etc_path.clone()))?;
            CString::new(etc_path_str).map_err(|_| InitError::InvalidPath(etc_path.clone()))?
        };

        let c_lib_path = {
            let lib_path_str = lib_path.to_str().ok_or_else(|| InitError::InvalidPath(lib_path.clone()))?;
            CString::new(lib_path_str).map_err(|_| InitError::InvalidPath(lib_path.clone()))?
        };

        let lib_rt = {
            let lib_rt_path = lib_path.join("libmono-2.0.dylib");
            let lib_rt_path_str = lib_rt_path.to_str().ok_or_else(|| InitError::InvalidPath(lib_rt_path.clone()))?;
            lib::Library::new(lib_rt_path_str).map_err(|e| InitError::FailedToLoadLibrary(e))?
        };

        let set_dirs = unsafe {
            let raw = &lib_rt as *const lib::Library;
            get_sym::<t_mono_set_dirs>(&*raw, "mono_set_dirs").map_err(|_| InitError::FailedToFindSymbol("mono_set_dirs".to_owned()))?
        };

        set_dirs(c_lib_path.as_ptr(), c_etc_path.as_ptr());

        let domain = {
            let c_domain_name = CString::new(domain_name).map_err(|_| InitError::InvalidDomainName(domain_name.to_owned()))?;

            let init_jit = unsafe {
                let raw = &lib_rt as *const lib::Library;
                get_sym::<t_mono_jit_init>(&*raw, "mono_jit_init").map_err(|_| InitError::FailedToFindSymbol("mono_jit_init".to_owned()))?
            };

            let raw = init_jit(c_domain_name.as_ptr());

            if raw.is_null() {
                return Err(InitError::FailedToCreateDomain);
            }

            MonoDomain { raw }
        };

        let sym_asm_open = unsafe {
            let raw = &lib_rt as *const lib::Library;
            get_sym::<t_mono_domain_assembly_open>(&*raw, "mono_domain_assembly_open").map_err(|_| InitError::FailedToFindSymbol("mono_domain_assembly_open".to_owned()))?
        };

        let sym_jit_execute = unsafe {
            let raw = &lib_rt as *const lib::Library;
            get_sym::<t_mono_jit_exec>(&*raw, "mono_jit_exec").map_err(|_| InitError::FailedToFindSymbol("mono_jit_exec".to_owned()))?
        };

        Ok(Self {
            etc_path,
            lib_path,
            domain,

            _lib: lib_rt,
            _sym_asm_open: sym_asm_open,
            _sym_jit_execute: sym_jit_execute,
        })
    }

    pub fn open_assembly<P: AsRef<Path>>(&self, asm_path: P) -> RtResult<MonoAssembly> {
        let path_str = asm_path.as_ref().to_str().ok_or_else(|| RtError::InvalidAssemblyPath)?;
        let c_asm_path = CString::new(path_str).map_err(|_| RtError::DataContainsNulByte)?;

        let raw = (self._sym_asm_open)(self.domain.raw, c_asm_path.as_ptr());
        if raw.is_null() {
            Err(RtError::RetValWasNull)
        } else {
            Ok(MonoAssembly { raw })
        }
    }

    pub fn execute(&self, asm: &MonoAssembly) -> RtResult<i32> {
        let args = &["/invocation/path", "Arg1", "Arg2", "Arg3"];
        let c_args = args.iter().map(|arg| CString::new(*arg).unwrap()).collect::<Vec<_>>();
        let c_arg_ptrs = c_args.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();

        let exit_code = (self._sym_jit_execute)(self.domain.raw, asm.raw, c_arg_ptrs.len() as i32, c_arg_ptrs.as_ptr()) as i32;
        Ok(exit_code)
    }
}