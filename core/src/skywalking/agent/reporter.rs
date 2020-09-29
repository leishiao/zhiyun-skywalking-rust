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
use crate::remote::trace_segment_report_service_client::TraceSegmentReportServiceClient;
use crate::remote::{KeyStringValuePair, SegmentObject, SpanObject};
use crate::skywalking::core::{ContextListener, TracingContext};
use futures_util::stream;
use log::error;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tonic::transport::Channel;
use tonic::Request;

#[allow(dead_code)]
pub struct Reporter {
    sender: Sender<Box<TracingContext>>,
    config: Config,
}

impl ContextListener for Reporter {
    // 这里使用ipv4的信息转化为instance_id
    fn service_instance_id(&self) -> Option<i32> {
        Some(self.config.instance_id)
    }

    // 这里不会阻塞,每次失败,重试次数+1
    fn report_trace(&mut self, trace_info: Box<TracingContext>, try_times: u8) -> bool {
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
                TrySendError::Closed(ctx) => {
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

    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::channel::<Box<TracingContext>>(100000);
        let config = Config::new();
        let run = Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .expect("create tokio runtime fail");
        run.spawn(async move {
            let config = config.clone();
            let tracing_client: Result<TraceSegmentReportServiceClient<Channel>, _> =
                TraceSegmentReportServiceClient::connect(format!(
                    "http://{}:{}",
                    config
                        .collector_host
                        .clone()
                        .unwrap_or("127.0.0.1".to_owned()),
                    config.collector_port.clone().unwrap_or("11800".to_owned())
                ))
                .await;
            if let Ok(mut client) = tracing_client {
                let mut segments_buffer = Vec::new();
                // loop {
                let ref_mut: &mut TraceSegmentReportServiceClient<Channel> = &mut client;
                let ctx_res = receiver.recv().await;
                // here we consume msg forever
                if let Some(ctx) = ctx_res {
                    let segment = Self::gen_segment(&ctx, &config);
                    segments_buffer.push(segment);
                }
                if segments_buffer.len() > 100 {
                    let request = Request::new(stream::iter(segments_buffer));
                    if let Err(e) = ref_mut.collect(request).await {
                        error!("send segment infos to skywalking failed, e:{:?}", e);
                    }
                }
            }
        });
        Reporter {
            config: Config::new(),
            sender,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_send_segs() {
        let reporter = Reporter::new();
        println!("config is:{:?}", reporter.config);
        println!("Report init finished!");
    }
}
