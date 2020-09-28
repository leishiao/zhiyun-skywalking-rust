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
use crate::remote::{KeyStringValuePair, SegmentObject, SegmentReference, SpanObject, SpanType};
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
        let config = Config::new();
        // start a consumer doing consuming work
        thread::spawn(move || loop {
            let config = config.clone();
            // gen segment object from tracing context
            fn gen_segment(ctx: &Box<TracingContext>, config: &Config) -> SegmentObject {
                let mut spans: Vec<SpanObject> = Vec::new();
                for s in &ctx.finished_spans {
                    let span_type = {
                        if s.is_exit() {
                            1 //exit
                        } else if s.is_entry() {
                            0 //entry
                        } else {
                            2 //local
                        }
                    };
                    let key_tags = {
                        let mut v = vec![];
                        for t in s.tags() {
                            v.push(KeyStringValuePair {
                                key: t.key(),
                                value: t.value(),
                            });
                        }
                        v
                    };
                    let span = SpanObject {
                        span_id: s.span_id(),
                        parent_span_id: s.parent_span_id(),
                        start_time: s.time_info().0 as i64,
                        end_time: s.time_info().1 as i64,
                        refs: Vec::new(),
                        operation_name: s.operation_name().to_owned(),
                        peer: s.peer_info().to_owned(),
                        span_type: span_type,
                        span_layer: s.span_layer().val(),
                        component_id: 94,
                        is_error: s.is_error(),
                        tags: key_tags,
                        logs: Vec::new(),
                        skip_analysis: false,
                    };
                    spans.push(span);
                }
                let segment = SegmentObject {
                    trace_id: ctx.trace_id().to_string(),
                    trace_segment_id: ctx.segment_id().to_string(),
                    spans,
                    service: config.service_name.to_owned(),
                    service_instance: config.service_instance.to_owned(),
                    is_size_limited: false,
                };
                segment
            }
            //

            let ctx_res = receiver.recv();
            // here we consume msg forever
            if let Ok(ctx) = ctx_res {
                let segment = gen_segment(&ctx, &config);
            }
        });
        Reporter {
            config: Config::new(),
            sender: sender,
        }
    }
}
