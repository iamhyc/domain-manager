extern crate libc;
extern crate libloading;

use std::path::Path;
use std::collections::HashMap;
use threadpool::ThreadPool;
//
use libc::{c_char};
use std::ffi::{CStr, CString};
use pyo3::prelude::*;
use serde_json::{self, Value};
//
use crate::shared_consts::VDM_CAPABILITY_DIR;

// - On-demand load cdll library with "usage count"
//      - load from "~/.vdm/capability" using "ffi.rs"
//      - with "register" command, +1; with "unregister" command, -1
//      - zero for release

type _PyFunc = String;
type _PyLibCode = String;

// #[repr(u8)]
enum RawFunc<'a,T,R>
{
    Value0(libloading::Symbol<'a, extern fn()->R>),
    Value1(libloading::Symbol<'a, extern fn(T)->R>),
    Value2(libloading::Symbol<'a, extern fn(T,T)->R>),
    Value3(libloading::Symbol<'a, extern fn(T,T,T)->R>),
    Value4(libloading::Symbol<'a, extern fn(T,T,T,T)->R>),
    Value5(libloading::Symbol<'a, extern fn(T,T,T,T,T)->R>)
}

impl<'a,T,R> RawFunc<'a,T,R> {
    pub fn load<'lib>(lib:&'lib libloading::Library, name:&[u8], len:usize) -> Option<RawFunc<'lib,T,R>> {
        match len {
            0 => {
                if let Ok(sym) = unsafe{ lib.get(name) } {
                    Some(RawFunc::Value0(sym))
                } else {None}
            },
            1 => {
                if let Ok(sym) = unsafe{ lib.get(name) } {
                    Some(RawFunc::Value1(sym))
                } else {None}
            },
            2 => {
                if let Ok(sym) = unsafe{ lib.get(name) } {
                    Some(RawFunc::Value2(sym))
                } else {None}
            },
            3 => {
                if let Ok(sym) = unsafe{ lib.get(name) } {
                    Some(RawFunc::Value3(sym))
                } else {None}
            },
            4 => {
                if let Ok(sym) = unsafe{ lib.get(name) } {
                    Some(RawFunc::Value4(sym))
                } else {None}
            },
            5 => {
                if let Ok(sym) = unsafe{ lib.get(name) } {
                    Some(RawFunc::Value5(sym))
                } else {None}
            },
            _ => {None}
        }
    }

    pub fn call(&self, mut args:Vec<T>) -> Result<R, Box<dyn std::error::Error>>{
        let mut iter = args.drain(..);
        match self {
            Self::Value0(func) => {
                Ok(
                    func()
                )
            },
            Self::Value1(func) => {
                Ok(
                    func( iter.next().unwrap() )
                )
            },
            Self::Value2(func) => {
                Ok(
                    func( iter.next().unwrap(), iter.next().unwrap() )
                )
            },
            Self::Value3(func) => {
                Ok(
                    func( iter.next().unwrap(), iter.next().unwrap(), iter.next().unwrap() )
                )
            },
            Self::Value4(func) => {
                Ok(
                    func( iter.next().unwrap(), iter.next().unwrap(), iter.next().unwrap(),
                          iter.next().unwrap(), )
                )
            },
            Self::Value5(func) => {
                Ok(
                    func( iter.next().unwrap(), iter.next().unwrap(), iter.next().unwrap(),
                          iter.next().unwrap(), iter.next().unwrap(), )
                )
            }
        }
    }
}

enum Func<'a> {
    CFunc(RawFunc<'a,*const c_char, *const c_char>),
    RustFunc(RawFunc<'a,String, String>),
    PythonFunc(_PyFunc)
}

impl<'a> Func<'a> {
    pub fn new<'lib>(lib:&'lib LibraryContext, name:&String, len:usize) -> Option<Func<'lib>> {
        match lib {
            LibraryContext::cdll(lib) => {
                if let Some(func) = RawFunc::load(lib, name.as_bytes(), len) {
                    Some( Func::CFunc(func) )
                } else {None}
            },
            LibraryContext::rust(lib) => {
                if let Some(func) = RawFunc::load(lib, name.as_bytes(), len) {
                    Some( Func::RustFunc(func) )
                } else {None}
            }
            LibraryContext::python(lib) => {
                Some( Func::PythonFunc(name.clone()) )
            }
        }
    }

    pub fn call(&self, args:Vec<Value>) -> String {
        let args: Vec<&Value> = args.iter().map(|arg| {
            let obj = arg.as_object().unwrap();
            let val = obj.values().next().unwrap();
            val
        }).collect();

        match self {
            Self::CFunc(func) => {
                let args:Vec<CString> = args.iter().map(|arg|{
                    CString::new( arg.to_string() ).unwrap()
                }).collect();
                let _args:Vec<*const c_char> = args.iter().map( |arg| {arg.as_ptr()} ).collect();
                unsafe{
                    CStr::from_ptr( func.call(_args).unwrap() ).to_string_lossy().into_owned()
                }
            },
            Self::RustFunc(func) => {
                let args:Vec<String> = args.iter().map(|arg|{
                    serde_json::to_string(arg).unwrap()
                }).collect();
                match func.call(args) {
                    Ok(res) => res,
                    Err(_) => String::new()   
                }
            },
            Self::PythonFunc(func) => {
                String::new() //FIXME: not implemented
            }
        }
    }
}

enum LibraryContext {
    cdll(libloading::Library),
    rust(libloading::Library),
    python(_PyLibCode)
}

struct Library<'a> { //'a is lifetime of context
    context: LibraryContext,
    functions: HashMap<String, Func<'a>>
}

impl<'a> Library<'a> {
    pub fn new(_type:&str, url:&Path) -> Option<Library<'a>> {
        let context = match _type {
            "c" | "cpp" => {
                if let Ok(lib) = unsafe{ libloading::Library::new(url) } {
                    Some(LibraryContext::cdll(lib))
                } else { None }
            },
            "rust" => {
                if let Ok(lib) = unsafe{ libloading::Library::new(url) } {
                    Some(LibraryContext::cdll(lib))
                } else {None}
            }
            "python" => {
                None //FIXME: load code from file
            },
            _ => { None }
        };
        if let Some(context) = context {
            Some(Library{context, functions:HashMap::new()})
        } else {None}
    }

    pub fn load(&'a mut self, metadata:&Value) -> &'a Self {
        for (key,val) in metadata.as_object().unwrap().iter() {
            let _args = &val.as_object().unwrap()["args"];
            let _len = _args.as_array().unwrap().len();
            if let Some(func) = Func::new(&self.context, &key, _len) {
                self.functions.insert(key.clone(), func);
            }
        }
        self
    }
}

pub struct FFIManager<'a> {
    root: String,
    pool: ThreadPool,
    library: HashMap<String, (u32, Library<'a>)>
}

impl<'a> FFIManager<'a> {
    pub fn new() -> FFIManager<'a> {
        FFIManager{
            root: shellexpand::tilde(VDM_CAPABILITY_DIR).into_owned(),
            pool: ThreadPool::new(num_cpus::get()),
            library: HashMap::new()
        }
    }

    pub fn preload(&mut self) {
        unimplemented!();
    }

    pub fn load(&mut self, manifest:&str) {
        unimplemented!();
    }

    pub fn register(&mut self, name: &str) -> Option<String> {
        unimplemented!();
    }

    pub fn unregister(&mut self, name: &str) {
        unimplemented!();
    }

    pub fn execute<T>(&self, raw_data:String, callback:T)
    where T: FnOnce(String) -> ()
    {
        let v: Value = serde_json::from_slice(raw_data.as_bytes()).unwrap();
        self.pool.execute(move || {
            let sig  = v["sig"].as_str().unwrap();
            let func = v["func"].as_str().unwrap();
            let ref args:Value = v["args"];
            unimplemented!();
        });
    }

    pub fn chain_execute<T>(&self, raw_data:String, callback:T)
    where T: FnOnce(String) -> ()
    {
        let v: Value = serde_json::from_slice(raw_data.as_bytes()).unwrap();
        self.pool.execute(move || {
            let ref sig_func_args_table:Value = v["sig_func_args_table"];
            unimplemented!();
        });
    }
}
