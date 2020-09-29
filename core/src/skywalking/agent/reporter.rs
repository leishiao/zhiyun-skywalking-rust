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
use lazy_static::lazy_static;
use log::error;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tonic::transport::Channel;
use tonic::Request;

const REPORT_BUCKET: u128 = 500;

lazy_static! {
    static ref GLOBAL_RT: Arc<Runtime> = Arc::new(
        Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .expect("create tokio runtime fail")
    );
    static ref GLOBAL_HANDLE: Arc<Handle> = Arc::new(GLOBAL_RT.handle().clone());
}

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
    fn report_trace(&self, trace_info: Box<TracingContext>, try_times: u8) -> bool {
        let mut sender = self.sender.clone();
        let result = sender.try_send(trace_info);
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
        GLOBAL_HANDLE.spawn(async move {
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
            // 每个经过一个time bucket, 执行一次flush操作
            if let Ok(mut client) = tracing_client {
                let mut segments_buffer = Vec::new();
                let mut last_flush_time = Instant::now();
                loop {
                    let ctx_res = receiver.recv().await;
                    // here we consume msg forever
                    if let Some(ctx) = ctx_res {
                        let segment = Self::gen_segment(&ctx, &config);
                        println!("received new segment:{:?}", segment);
                        segments_buffer.push(segment);
                    }
                    // test use
                    use tokio::time::delay_for;
                    delay_for(std::time::Duration::from_millis(REPORT_BUCKET as u64)).await;

                    if last_flush_time.elapsed().as_micros() >= REPORT_BUCKET {
                        println!("start to flush to grpc tunnel...");
                        let request = Request::new(stream::iter(segments_buffer));
                        if let Err(e) = client.collect(request).await {
                            error!("send segment infos to skywalking failed, e:{:?}", e);
                            println!("error! {:?}", e);
                        }
                        // 生成新的Vec来存储buffer的数据
                        segments_buffer = Vec::new();
                        // 刷新最后一次提交的时间为当前时间
                        last_flush_time = Instant::now();
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
    use crate::skywalking::agent::ContextManager;
    use crate::skywalking::core::SpanLayer;
    use std::{thread, time};

    #[test]
    fn test_send_segs() {
        let mut run = Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .expect("create tokio runtime fail");
        run.block_on(async move {
            let _ = ContextManager::async_enter(async { execute().await }).await;
        });
        let ten_millis = time::Duration::from_secs(2);
        thread::sleep(ten_millis);
    }

    // 异步执行测试方法
    async fn execute() {
        let entry_span = ContextManager::tracing_entry("/xxx/xxx", SpanLayer::HTTP, None);
        let exit_span =
            ContextManager::tracing_exit("rpc.service:1.0", "127.0.0.1", SpanLayer::Rpc, None);
        ContextManager::finish_span(exit_span);
        ContextManager::finish_span(entry_span);
    }
}
