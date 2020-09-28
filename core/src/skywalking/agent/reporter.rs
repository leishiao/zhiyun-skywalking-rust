// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::config::Config;
// use crate::remote::*;
use crate::remote::trace_segment_report_service_client::*;
use crate::remote::*;
use crate::skywalking::core::{ContextListener, TracingContext};
use log::error;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::TrySendError;
use std::thread;

#[allow(dead_code)]
pub struct Reporter {
    sender: SyncSender<Box<TracingContext>>,
    config: Config,
}

impl ContextListener for Reporter {
    // 这里使用ipv4的信息转化为instance_id
    fn service_instance_id(&self) -> Option<i32> {
        Some(self.config.instance_id)
    }

    // 这里不会阻塞,每次失败,重试次数+1
    fn report_trace(&self, trace_info: Box<TracingContext>, try_times: u8) -> bool {
        let result = self.sender.try_send(trace_info);
        if let Err(e) = result {
            match e {
                TrySendError::Full(ctx) => {
                    if try_times > 3 {
                        error!(
                            "skywalking channel is full!traceId:{:?} msg log to disk.",
                            ctx.trace_id()
                        );
                        error!("tracing_ctx_info:{:?}", ctx);
                    }
                }
                TrySendError::Disconnected(ctx) => {
                    error!(
                        "skywalking channel is disconnected! searious error !traceId:{:?} msg log to disk.",
                        ctx.trace_id()
                    );
                    error!("tracing_ctx_info:{:?}", ctx);
                }
            }
            return false;
        }
        true
    }
}

impl Reporter {
    pub fn new() -> Self {
        let (sender, receiver) = sync_channel::<Box<TracingContext>>(100000);
        // start a consumer doing consuming work
        thread::spawn(move || loop {
            let _ = receiver.recv();
            // here we consume msg forever
        });
        Reporter {
            config: Config::new(),
            sender: sender,
        }
    }
}
