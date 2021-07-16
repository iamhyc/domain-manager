use std::path::PathBuf;
use std::{io, fs};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
// third-party crates
use shellexpand::tilde as expand_user;
use tokio::runtime::Runtime as TokioRuntime;
use serde_json::{self, Value as JsonValue};
// root crates
use crate::core::ipc;
use crate::core::ffi;
use crate::core::traits::{Serde, IPCProtocol};
use crate::core::command::ExecResult;

pub struct JsonifyIPC<P>
where P: IPCProtocol
{
    // root: PathBuf,
    server_port: u16,
    rt: TokioRuntime,
    ffi: ffi::ArcFFIManager,
    server: Option<Arc<Mutex<ipc::IPCServer<P>>>>
}

impl Serde for ffi::FFIManager
{
    type Value = JsonValue;

    fn to_raw_data(v:&JsonValue) -> Option<String> {
        serde_json::to_string(v).ok()
    }

    fn from_raw_data<T>(r:&T) -> Option<JsonValue> 
    where T: Into<JsonValue> + Clone
    {
        Some( r.clone().into() )
        // serde_json::from_str(r).ok()
    }
}

impl<P> JsonifyIPC<P>
where P: IPCProtocol
{
    /// Return JsonifyIPC handle configured with given:
    /// - (Optional) **path**: the working directory for capability, default is `~/.vdm/libs`
    pub fn new(root:Option<String>, server_port:Option<u16>) -> Self {
        let root = PathBuf::from(
            root.unwrap_or( expand_user("~/.serde_ipc").into_owned() )
        );
        let server_port = server_port.unwrap_or(42000);

        let rt = TokioRuntime::new().unwrap();
        let ffi = Arc::new(Mutex::new(
            ffi::FFIManager::new(root)
        ));
        
        JsonifyIPC {
            server_port, rt, ffi, server:None
        }
    }

    /// Start the JsonifyIPC daemon waiting for client connection.
    pub fn start(&mut self) {
        let ffi = self.ffi.clone();

        self.server = Some( ipc::IPCServer::<P>::new(
            self.server_port, ffi
        ) );

        let _server = self.server.clone();
        self.rt.spawn(async move {
            ipc::IPCServer::daemon( _server.unwrap() ).await
        });
    }

    /// Stop the JsonifyIPC daemon by: 1) shutdown all tokio threads; 2) stop IPCServer thread pool.
    pub fn stop(mut self) {
        self.rt.shutdown_background(); //drop occurs here
        self.server = None; //drop occurs here
        //
        self.rt = TokioRuntime::new().unwrap();
    }

    /// Add service via FFI Manager
    pub fn install_service(&self, src_path:String) -> ExecResult {
        let directory = PathBuf::from(src_path);
        let manifest = fs::File::open( directory.join("manifest.json") )
                        .or( Err(format!("'manifest.json' file not found.")) )?;
        let manifest:JsonValue = serde_json::from_reader( io::BufReader::new(manifest) )
                        .or( Err(format!("manifest file load failed.")) )?;
        
        
        let build: ffi::BuildTemplate = manifest.get("build").and_then(|val|{
                serde_json::from_value( val.clone() ).ok()
        }).ok_or( format!("'build' section missing in manifest file.") )?;

        let runtime: ffi::RuntimeTemplate = manifest.get("runtime").and_then(|val|{
                serde_json::from_value( val.clone() ).ok()
        }).ok_or( format!("'runtime' section missing in manifest file.") )?;

        let mut metadata = ffi::Metadata {
            name: manifest.get("name").and_then( |val|{val.as_str()} )
                    .ok_or( format!("'name' section missing ins manifest file.") )?.into(),
            class: manifest.get("type").and_then( |val|{val.as_str()} )
                    .ok_or( format!("'type' section missing ins manifest file.") )?.into(),
            version: manifest.get("version").and_then( |val|{val.as_str()} )
                    .ok_or( format!("'version' section missing ins manifest file.") )?.into(),
            func: HashMap::new()
        };

        //TODO: feed in MetaFunc
        

        let _ffi = self.ffi.lock().unwrap();
        _ffi.install(directory, metadata, build, runtime)
    }

    /// Remove service via FFI Manager
    pub fn uninstall_service(&self, name:String) -> ExecResult {
        let _ffi = self.ffi.lock().unwrap();
        _ffi.uninstall(&name)
    }

    /// Get service directly via FFI Manager
    pub fn get_service(&mut self, name:String) -> Option<String> {
        let mut _ffi = self.ffi.lock().ok()?;
        _ffi.register(&name)
    }

    /// Destroy service directly via FFI Manager
    pub fn put_service(&mut self, name:String, srv_use_sig: String) {
        let mut _ffi = self.ffi.lock().unwrap();
        _ffi.unregister(&name, &srv_use_sig);
    }
}
