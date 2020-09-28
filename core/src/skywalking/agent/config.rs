use crate::skywalking::agent::util::get_inst_default_ip;
use rand::RngCore;
use std::env::var;
use uuid::Uuid;

#[allow(dead_code)]
const APP_NAME_KEY: &str = "APP_NAME";
#[allow(dead_code)]
const UNKNOWN_SERVICE: &str = "UNKNOWN_SERVICE";
#[allow(dead_code)]
const SKW_SER_INST_KEY: &str = "SKW_SER_INST_KEY";
#[allow(dead_code)]
const SKW_COLLECTOR_HOST_KEY: &str = "SKW_COLLECTOR_HOST_KEY";
#[allow(dead_code)]
const SKW_COLLECTOR_PORT_KEY: &str = "SKW_COLLECTOR_PORT_KEY";

#[derive(Debug, Clone)]
pub struct Config {
    // OAP的采集服务的地址
    pub collector_host: Option<String>,
    pub collector_port: Option<String>,
    // 应用的名称,通过环境变量获取，如果获取不到就使用UNKNOWN_SERVICE作为其默认值
    pub service_name: String,
    // 每一次机器重启之后都会发生变化,可以通过环境变量去指定
    pub service_instance: String,
    pub instance_id: i32,
}

impl Config {
    pub fn new() -> Config {
        let service_name = if let Ok(v) = var(APP_NAME_KEY) {
            v
        } else {
            UNKNOWN_SERVICE.to_owned()
        };
        let service_instance = if let Ok(v) = var(SKW_SER_INST_KEY) {
            v
        } else {
            let uuid = Uuid::new_v4();
            let current_inst_ip = match get_inst_default_ip() {
                Ok(s) => s,
                Err(_) => "Unknown".to_owned(),
            };

            format!("{}@{}", uuid.to_string().replace("-", ""), current_inst_ip)
        };

        let collector_host = if let Ok(v) = var(SKW_COLLECTOR_HOST_KEY) {
            Some(v)
        } else {
            Some("127.0.0.1".to_owned())
        };
        let collector_port = if let Ok(v) = var(SKW_COLLECTOR_PORT_KEY) {
            Some(v)
        } else {
            Some("11800".to_owned())
        };

        let inst_id = {
            let inst_id = rand::thread_rng().next_u32();
            i32::from_be_bytes(inst_id.to_be_bytes())
        };
        Config {
            collector_host,
            collector_port,
            service_name: service_name,
            service_instance: service_instance,
            instance_id: inst_id,
        }
    }
}
