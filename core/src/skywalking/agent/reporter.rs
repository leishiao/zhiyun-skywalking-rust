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
use anyhow::Result;
use futures_util::stream;
use lazy_static::lazy_static;
use log::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tokio::time::delay_for;
use tonic::transport::Channel;
use tonic::Code;
use tonic::Request;

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
    pub config: Config,
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
                let mut v = Vec::new();
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
            let mut client = Self::loop_retrive_client(&config).await;
            // 每个经过一个time bucket, 执行一次flush操作
            loop {
                let ctx_res = receiver.recv().await;
                debug!("has received new msg:{:?}", ctx_res);
                Self::send_to_grpc_trace_service(&mut client, &config, ctx_res).await;
            }
        });
        Reporter {
            config: Config::new(),
            sender,
        }
    }

    // retry connect operation until get a correct trace segment client
    // it will retry until the end of the world
    pub async fn loop_retrive_client(config: &Config) -> TraceSegmentReportServiceClient<Channel> {
        loop {
            let client_res = Self::retrieve_client(config).await;
            if let Ok(client) = client_res {
                return client;
            }
            delay_for(Duration::from_secs(1)).await;
        }
    }

    // 特定情况下失败重试，重试3次后放弃，进行日志备份到本地磁盘
    pub async fn send_to_grpc_trace_service(
        client: &mut TraceSegmentReportServiceClient<Channel>,
        config: &Config,
        ctx_res: Option<Box<TracingContext>>,
    ) {
        let segments = Self::transform_segments(config, &ctx_res);
        let request = Request::new(stream::iter(segments));
        if let Err(e) = client.collect(request).await {
            match e.code() {
                // 下面这几种情况进行重试操作:
                // 1.DeadlineExceeded: 请求发生了超时
                // 2.ResourceExhausted: 资源临时性耗尽
                // 3.Unavailable: 服务暂时性不可用
                Code::DeadlineExceeded | Code::ResourceExhausted | Code::Unavailable => {
                    for _ in 0..3 {
                        let segments = Self::transform_segments(config, &ctx_res);
                        let request = Request::new(stream::iter(segments));
                        // success return
                        if let Ok(_) = client.collect(request).await {
                            debug!("segments send to grpc success:{:?}", ctx_res);
                            return;
                        }
                        delay_for(Duration::from_micros(500)).await;
                    }
                }
                _ => {
                    error!("skywalking trace collect method has met serious exception.segments backup to disk.{:?}", 
                        Self::transform_segments(config, &ctx_res));
                }
            }
        }
    }

    // segments type transform from library format to grpc format
    pub fn transform_segments(
        config: &Config,
        ctx: &Option<Box<TracingContext>>,
    ) -> Vec<SegmentObject> {
        let mut segments_buffer = Vec::new();
        if let Some(ctx) = ctx {
            let segment = Self::gen_segment(ctx, &config);
            // println!("Sent segment is:{:?}", segment);
            segments_buffer.push(segment);
        }
        segments_buffer
    }

    pub async fn retrieve_client(
        config: &Config,
    ) -> Result<TraceSegmentReportServiceClient<Channel>> {
        let tracing_client = TraceSegmentReportServiceClient::connect(format!(
            "http://{}:{}",
            config
                .collector_host
                .clone()
                .unwrap_or("127.0.0.1".to_owned()),
            config.collector_port.clone().unwrap_or("11800".to_owned())
        ))
        .await?;
        Ok(tracing_client)
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
            // async { execute().await }.await
        });
        let ten_millis = time::Duration::from_secs(2);
        thread::sleep(ten_millis);
    }

    #[test]
    fn test_not_set_async_env() {
        let mut run = Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .expect("create tokio runtime fail");
        run.block_on(async move { async { execute().await }.await });
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
