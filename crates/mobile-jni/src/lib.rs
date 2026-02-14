use jni::objects::{GlobalRef, JClass, JString, JValue};
use jni::sys::jstring;
use jni::{JNIEnv, JavaVM};
use log::LevelFilter;
use phoneclaw_tools::android_tools::AndroidBridge;
use std::path::PathBuf;
use std::sync::{Arc, Once};
use std::thread;
use async_trait::async_trait;

static INIT: Once = Once::new();

struct AndroidBridgeImpl {
    vm: JavaVM,
    bridge_class: GlobalRef,
}

impl AndroidBridgeImpl {
    fn new(vm: JavaVM, bridge_class: GlobalRef) -> Self {
        Self { vm, bridge_class }
    }

    fn call_bool_method(&self, method: &str, sig: &str, args: &[JValue]) -> Result<bool, String> {
        let mut env = self.vm.attach_current_thread_permanently().map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(&self.bridge_class, method, sig, args)
            .map_err(|e| e.to_string())?;
        result.z().map_err(|e| e.to_string())
    }
}

#[async_trait]
impl AndroidBridge for AndroidBridgeImpl {
    async fn click(&self, x: f32, y: f32) -> Result<bool, String> {
        self.call_bool_method("performClick", "(FF)Z", &[x.into(), y.into()])
    }

    async fn scroll(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> Result<bool, String> {
        self.call_bool_method("performScroll", "(FFFF)Z", &[x1.into(), y1.into(), x2.into(), y2.into()])
    }

    async fn back(&self) -> Result<bool, String> {
        self.call_bool_method("performBack", "()Z", &[])
    }

    async fn home(&self) -> Result<bool, String> {
        self.call_bool_method("performHome", "()Z", &[])
    }

    async fn input_text(&self, text: String) -> Result<bool, String> {
        let mut env = self.vm.attach_current_thread_permanently().map_err(|e| e.to_string())?;
        let jtext = env.new_string(text).map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(&self.bridge_class, "performInputText", "(Ljava/lang/String;)Z", &[(&jtext).into()])
            .map_err(|e| e.to_string())?;
        result.z().map_err(|e| e.to_string())
    }

    async fn dump_hierarchy(&self) -> Result<String, String> {
        let mut env = self.vm.attach_current_thread_permanently().map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(&self.bridge_class, "performDumpHierarchy", "()Ljava/lang/String;", &[])
            .map_err(|e| e.to_string())?;
        let jstr = result.l().map_err(|e| e.to_string())?;
        let jstring: JString = jstr.into();
        let rust_str: String = env.get_string(&jstring).map_err(|e| e.to_string())?.into();
        Ok(rust_str)
    }

    async fn screenshot(&self) -> Result<Vec<u8>, String> {
        let mut env = self.vm.attach_current_thread_permanently().map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(&self.bridge_class, "performTakeScreenshot", "()[B", &[])
            .map_err(|e| e.to_string())?;
        let jobj = result.l().map_err(|e| e.to_string())?;
        let jbyte_array: jni::objects::JByteArray = jobj.into();
        let bytes = env.convert_byte_array(jbyte_array).map_err(|e| e.to_string())?;
        Ok(bytes)
    }
}

#[no_mangle]
pub extern "system" fn Java_com_phoneclaw_app_RustBridge_startServer(
    mut env: JNIEnv,
    class: JClass,
    config_path: JString,
) -> jstring {
    // Initialize Android logger once
    INIT.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(LevelFilter::Info)
                .with_tag("PhoneClaw"),
        );
    });

    // Convert Java string to Rust PathBuf
    let config_path_str: String = env
        .get_string(&config_path)
        .expect("Couldn't get java string!")
        .into();
    let config_path = PathBuf::from(config_path_str);
    if let Some(config_dir) = config_path.parent() {
        let approved_path = config_dir.join("approved_skills.json");
        std::env::set_var(
            "PHONECLAW_APPROVED_SKILLS_PATH",
            approved_path.to_string_lossy().to_string(),
        );
    }

    // Capture JavaVM and Class Reference
    let vm = env.get_java_vm().expect("Failed to get JavaVM");
    let bridge_class = env.new_global_ref(class).expect("Failed to create GlobalRef");
    let bridge = Arc::new(AndroidBridgeImpl::new(vm, bridge_class));

    log::info!("Starting PhoneClaw Server with config: {:?}", config_path);

    // Spawn the server in a new thread because start_server blocks
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async move {
            // Pass the bridge to start_server
            if let Err(e) = phoneclaw_cli::start_server(Some(config_path), Some(bridge)).await {
                log::error!("Server failed: {}", e);
            }
        });
    });

    let output = env
        .new_string("Server started")
        .expect("Couldn't create java string!");
    output.into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_phoneclaw_app_RustBridge_stopServer(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    // TODO: Implement graceful shutdown mechanism in phoneclaw-cli first
    log::info!("Stop server requested (not fully implemented)");
    
    let output = env
        .new_string("Stop signal sent")
        .expect("Couldn't create java string!");
    output.into_raw()
}
